import {
  Injectable,
  NotFoundException,
  ForbiddenException,
  BadRequestException,
} from '@nestjs/common';
import { InjectRepository } from '@nestjs/typeorm';
import { EntityManager, Repository } from 'typeorm';
import { BoardColumn } from './entities/column.entity';
import { Board } from '../boards/entities/board.entity';
import { Card } from '../cards/entities/card.entity';
import { CreateColumnDto } from './dto/create-column.dto';
import { UpdateColumnDto } from './dto/update-column.dto';
import {
  attachChecklistProgress,
  attachDescriptionFlags,
  buildCardSummaryQuery,
} from '../common/card-summary.helpers';

@Injectable()
export class ColumnsService {
  constructor(
    @InjectRepository(BoardColumn)
    private readonly columnRepository: Repository<BoardColumn>,
    @InjectRepository(Board)
    private readonly boardRepository: Repository<Board>,
    @InjectRepository(Card)
    private readonly cardRepository: Repository<Card>,
  ) {}

  async findAllByBoardId(boardId: number, userId: number): Promise<BoardColumn[]> {
    const board = await this.boardRepository.findOne({
      where: { id: boardId, user_id: userId },
    });

    if (!board) {
      throw new NotFoundException('Board not found');
    }

    const columns = await this.columnRepository.find({
      where: { board_id: boardId },
      order: { position: 'ASC' },
    });

    if (columns.length === 0) {
      return columns;
    }

    const columnIds = columns.map((c) => c.id);
    const cardsByColumn = await this.findSummaryCardsByColumnIds(columnIds);

    for (const column of columns) {
      column.cards = (cardsByColumn.get(column.id) ?? []).sort((a, b) => a.position - b.position);
    }

    await this.enrichWithProgress(columns);

    return columns;
  }

  // List-query helper: loads card summaries without SELECTing description TEXT.
  // has_description comes from SQL CASE (boolean only); progress via batch aggregation.
  private async findSummaryCardsByColumnIds(columnIds: number[]): Promise<Map<number, Card[]>> {
    if (columnIds.length === 0) {
      return new Map<number, Card[]>();
    }

    const cards = await buildCardSummaryQuery(this.cardRepository)
      .where('card.column_id IN (:...columnIds)', { columnIds })
      .orderBy('card.position', 'ASC')
      .getMany();

    if (cards.length > 0) {
      await attachDescriptionFlags(this.cardRepository, cards);
    }

    const byColumn = new Map<number, Card[]>();
    for (const card of cards) {
      const list = byColumn.get(card.column_id) ?? [];
      list.push(card);
      byColumn.set(card.column_id, list);
    }
    return byColumn;
  }

  private async enrichWithProgress(columns: BoardColumn[]): Promise<void> {
    const cards = columns.flatMap((col) => col.cards);
    await attachChecklistProgress(this.cardRepository, cards);
  }

  async create(boardId: number, userId: number, dto: CreateColumnDto): Promise<BoardColumn> {
    const board = await this.boardRepository.findOne({
      where: { id: boardId, user_id: userId },
    });

    if (!board) {
      throw new NotFoundException('Board not found');
    }

    const maxPositionResult = await this.columnRepository
      .createQueryBuilder('column')
      .where('column.board_id = :boardId', { boardId })
      .select('MAX(column.position)', 'max')
      .getRawOne<{ max: string | number | null }>();

    const maxPosition = Number(maxPositionResult?.max ?? -1);

    const column = this.columnRepository.create({
      name: dto.name || 'New Column',
      position: maxPosition + 1,
      board_id: boardId,
    });

    return this.columnRepository.save(column);
  }

  async update(id: number, userId: number, dto: UpdateColumnDto): Promise<BoardColumn> {
    const column = await this.findOneById(id, userId);

    if (dto.name !== undefined) {
      column.name = dto.name;
    }

    return this.columnRepository.save(column);
  }

  async remove(id: number, userId: number): Promise<void> {
    const column = await this.findOneById(id, userId);
    await this.columnRepository.remove(column);
  }

  private async findOneById(id: number, userId: number): Promise<BoardColumn> {
    const column = await this.columnRepository.findOne({
      where: { id },
      relations: ['board'],
    });

    if (!column) {
      throw new NotFoundException('Column not found');
    }

    if (column.board.user_id !== userId) {
      throw new ForbiddenException('Access denied');
    }

    return column;
  }

  async sortCards(id: number, userId: number, order: 'asc' | 'desc'): Promise<BoardColumn> {
    await this.findOneById(id, userId);

    const cards = await this.cardRepository.find({
      where: { column_id: id },
      order: { created_at: order.toUpperCase() as 'ASC' | 'DESC' },
    });

    for (let i = 0; i < cards.length; i++) {
      cards[i].position = i;
    }
    await this.cardRepository.save(cards);

    const sortedColumn = await this.columnRepository.findOne({
      where: { id },
    });
    if (!sortedColumn) {
      throw new NotFoundException('Column not found');
    }

    const cardsByColumn = await this.findSummaryCardsByColumnIds([id]);
    sortedColumn.cards = (cardsByColumn.get(id) ?? []).sort((a, b) => a.position - b.position);
    await this.enrichWithProgress([sortedColumn]);
    return sortedColumn;
  }

  async moveAllCards(
    sourceId: number,
    targetId: number,
    userId: number,
  ): Promise<{ movedCount: number; targetName: string }> {
    const sourceColumn = await this.findOneById(sourceId, userId);
    const targetColumn = await this.findOneById(targetId, userId);

    if (sourceColumn.board_id !== targetColumn.board_id) {
      throw new BadRequestException('Cannot move cards between different boards');
    }

    if (sourceId === targetId) {
      throw new BadRequestException('Cards already in this column');
    }

    const cards = await this.cardRepository.find({
      where: { column_id: sourceId },
    });

    if (cards.length === 0) {
      throw new BadRequestException('No cards to move');
    }

    await this.cardRepository.manager.transaction(async (manager: EntityManager) => {
      const maxPositionResult = await manager
        .createQueryBuilder(Card, 'card')
        .where('card.column_id = :targetId', { targetId })
        .select('MAX(card.position)', 'max')
        .getRawOne<{ max: string | number | null }>();

      let maxPosition = Number(maxPositionResult?.max ?? -1);

      for (const card of cards) {
        card.column_id = targetId;
        card.position = ++maxPosition;
      }

      await manager.save(cards);
    });

    return { movedCount: cards.length, targetName: targetColumn.name };
  }
}
