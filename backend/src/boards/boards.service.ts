import { Injectable, NotFoundException, ForbiddenException } from '@nestjs/common';
import { InjectRepository } from '@nestjs/typeorm';
import { Repository } from 'typeorm';
import { Board } from './entities/board.entity';
import { BoardColumn } from '../columns/entities/column.entity';
import { Project } from '../projects/entities/project.entity';
import { CreateBoardDto } from './dto/create-board.dto';
import { UpdateBoardDto } from './dto/update-board.dto';

@Injectable()
export class BoardsService {
  constructor(
    @InjectRepository(Board)
    private readonly boardRepository: Repository<Board>,
    @InjectRepository(BoardColumn)
    private readonly columnRepository: Repository<BoardColumn>,
    @InjectRepository(Project)
    private readonly projectRepository: Repository<Project>,
  ) {}

  async findAllByUserId(
    userId: number,
    projectId?: number,
    includeArchived = false,
  ): Promise<Board[]> {
    const where: Record<string, number | boolean> = { user_id: userId };
    if (projectId) {
      where.project_id = projectId;
    }
    if (!includeArchived) {
      where.is_archived = false;
    }
    return this.boardRepository.find({
      where,
      relations: ['project'],
      order: { updated_at: 'DESC' },
    });
  }

  async findAllArchivedByUserId(userId: number): Promise<Board[]> {
    return this.boardRepository.find({
      where: { user_id: userId, is_archived: true },
      relations: ['project'],
      order: { updated_at: 'DESC' },
    });
  }

  async findOne(id: number, userId: number): Promise<Board> {
    const board = await this.boardRepository.findOne({
      where: { id, user_id: userId },
      relations: ['project', 'columns'],
    });

    if (!board) {
      throw new NotFoundException('Board not found');
    }

    return board;
  }

  async create(userId: number, dto: CreateBoardDto): Promise<Board> {
    const board = this.boardRepository.create({
      name: dto.name,
      background_color: dto.background_color || '#0079BF',
      user_id: userId,
      project_id: dto.project_id || null,
    });

    const savedBoard = await this.boardRepository.save(board);

    const defaultColumns = [
      { name: 'To Do', position: 0, board_id: savedBoard.id },
      { name: 'In Progress', position: 1, board_id: savedBoard.id },
      { name: 'Done', position: 2, board_id: savedBoard.id },
    ];

    await this.columnRepository.save(this.columnRepository.create(defaultColumns));

    return this.findOne(savedBoard.id, userId);
  }

  async update(id: number, userId: number, dto: UpdateBoardDto): Promise<Board> {
    const board = await this.findOne(id, userId);

    if (dto.name !== undefined) {
      board.name = dto.name;
    }
    if (dto.background_color !== undefined) {
      board.background_color = dto.background_color;
    }
    if (dto.project_id !== undefined) {
      if (dto.project_id === null) {
        board.project_id = null;
        board.project = null;
      } else {
        const project = await this.projectRepository.findOne({
          where: { id: dto.project_id, user_id: userId },
        });
        if (!project) {
          throw new ForbiddenException('Project not found or access denied');
        }
        // Keep the FK column and the relation in sync: the entity maps both
        // `project_id` (@Column) and `project` (@ManyToOne on the same column),
        // and save() persists the FK from the relation side.
        board.project_id = project.id;
        board.project = project;
      }
    }
    if (dto.view_mode === 'board' || dto.view_mode === 'list') {
      board.view_mode = dto.view_mode;
    }

    return this.boardRepository.save(board);
  }

  async remove(id: number, userId: number): Promise<void> {
    const board = await this.findOne(id, userId);
    await this.boardRepository.remove(board);
  }

  async archive(id: number, userId: number): Promise<Board> {
    const board = await this.findOne(id, userId);
    if (board.is_archived) {
      return board;
    }
    board.is_archived = true;
    return this.boardRepository.save(board);
  }

  async restore(id: number, userId: number): Promise<Board> {
    const board = await this.findOne(id, userId);
    if (!board.is_archived) {
      return board;
    }
    board.is_archived = false;
    return this.boardRepository.save(board);
  }

  async permanentDelete(id: number, userId: number): Promise<void> {
    const board = await this.findOne(id, userId);
    if (!board.is_archived) {
      throw new ForbiddenException('Board must be archived before permanent deletion');
    }
    await this.boardRepository.remove(board);
  }
}
