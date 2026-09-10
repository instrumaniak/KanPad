import { Test, TestingModule } from '@nestjs/testing';
import { getRepositoryToken } from '@nestjs/typeorm';
import { NotFoundException, ForbiddenException } from '@nestjs/common';
import { NotesService } from './notes.service';
import { Note } from './entities/note.entity';
import { Tag } from '../tags/entities/tag.entity';
import { Board } from '../boards/entities/board.entity';
import { Project } from '../projects/entities/project.entity';
import { Card } from '../cards/entities/card.entity';
import { NoteType } from './dto/list-notes.dto';

describe('NotesService', () => {
  let service: NotesService;

  const mockUserId = 1;
  const mockDate = new Date('2026-01-01T00:00:00.000Z');

  const mockBoard = { id: 1, user_id: mockUserId } as Board;
  const mockProject = { id: 1, user_id: mockUserId } as Project;
  const mockColumn = {
    id: 1,
    board_id: 1,
    board: { id: 1, user_id: mockUserId },
  } as any;
  const mockCard = { id: 1, column_id: 1, column: mockColumn } as Card;
  const mockTag = { id: 1, name: 'important', color: 'teal', user_id: mockUserId } as Tag;
  const otherTag = { id: 2, name: 'urgent', color: 'red', user_id: 999 } as Tag;

  const mockNoteWithBoard = {
    id: 1,
    title: 'Board Note',
    content: 'Content',
    board_id: 1,
    project_id: null,
    card_id: null,
    user_id: mockUserId,
    board: { id: 1, user_id: mockUserId },
    project: null,
    card: null,
    tags: [mockTag],
    created_at: mockDate,
    updated_at: mockDate,
  } as Note;

  const mockNoteWithProject = {
    id: 2,
    title: 'Project Note',
    content: 'Content',
    board_id: null,
    project_id: 1,
    card_id: null,
    user_id: mockUserId,
    board: null,
    project: { id: 1, user_id: mockUserId },
    card: null,
    tags: [],
    created_at: mockDate,
    updated_at: mockDate,
  } as unknown as Note;

  const mockNoteWithCard = {
    id: 3,
    title: 'Card Note',
    content: 'Content',
    board_id: null,
    project_id: null,
    card_id: 1,
    user_id: mockUserId,
    board: null,
    project: null,
    card: { id: 1, column_id: 1, column: { board: { user_id: mockUserId } } },
    tags: [],
    created_at: mockDate,
    updated_at: mockDate,
  } as unknown as Note;

  const mockNoteDirect = {
    id: 4,
    title: 'Direct Note',
    content: 'Content',
    board_id: null,
    project_id: null,
    card_id: null,
    user_id: mockUserId,
    board: null,
    project: null,
    card: null,
    tags: [],
    created_at: mockDate,
    updated_at: mockDate,
  } as Note;

  const mockNoteRepository = {
    create: jest.fn(),
    save: jest.fn(),
    find: jest.fn(),
    findOne: jest.fn(),
    remove: jest.fn(),
    update: jest.fn(),
    createQueryBuilder: jest.fn(),
  };

  const mockTagRepository = {
    findBy: jest.fn(),
  };

  const mockBoardRepository = {
    findOne: jest.fn(),
  };

  const mockProjectRepository = {
    findOne: jest.fn(),
  };

  const mockCardRepository = {
    findOne: jest.fn(),
  };

  const mockQueryBuilder = {
    leftJoinAndSelect: jest.fn().mockReturnThis(),
    where: jest.fn().mockReturnThis(),
    andWhere: jest.fn().mockReturnThis(),
    orderBy: jest.fn().mockReturnThis(),
    take: jest.fn().mockReturnThis(),
    skip: jest.fn().mockReturnThis(),
    getMany: jest.fn(),
  };

  beforeEach(async () => {
    const module: TestingModule = await Test.createTestingModule({
      providers: [
        NotesService,
        { provide: getRepositoryToken(Note), useValue: mockNoteRepository },
        { provide: getRepositoryToken(Tag), useValue: mockTagRepository },
        { provide: getRepositoryToken(Board), useValue: mockBoardRepository },
        { provide: getRepositoryToken(Project), useValue: mockProjectRepository },
        { provide: getRepositoryToken(Card), useValue: mockCardRepository },
      ],
    }).compile();

    service = module.get<NotesService>(NotesService);
  });

  afterEach(() => {
    jest.clearAllMocks();
  });

  describe('create', () => {
    it('should create a note with tags', async () => {
      mockTagRepository.findBy.mockResolvedValue([mockTag]);
      mockBoardRepository.findOne.mockResolvedValue(mockBoard);
      mockNoteRepository.create.mockReturnValue(mockNoteWithBoard);
      mockNoteRepository.save.mockResolvedValue(mockNoteWithBoard);

      const result = await service.create(mockUserId, {
        title: 'Board Note',
        content: 'Content',
        board_id: 1,
        tagIds: [1],
      });

      expect(result).toBeDefined();
      expect(result.title).toBe('Board Note');
      expect(mockTagRepository.findBy).toHaveBeenCalled();
      expect(mockNoteRepository.create).toHaveBeenCalledWith({
        title: 'Board Note',
        content: 'Content',
        board_id: 1,
        project_id: null,
        card_id: null,
        user_id: mockUserId,
        tags: [mockTag],
      });
      expect(mockNoteRepository.save).toHaveBeenCalled();
    });

    it('should create a note without tags', async () => {
      mockNoteRepository.create.mockReturnValue(mockNoteDirect);
      mockNoteRepository.save.mockResolvedValue(mockNoteDirect);

      const result = await service.create(mockUserId, {
        title: 'Direct Note',
        content: 'Content',
      });

      expect(result).toBeDefined();
      expect(mockTagRepository.findBy).not.toHaveBeenCalled();
      expect(mockNoteRepository.create).toHaveBeenCalledWith({
        title: 'Direct Note',
        content: 'Content',
        board_id: null,
        project_id: null,
        card_id: null,
        user_id: mockUserId,
        tags: [],
      });
    });

    it('should throw ForbiddenException when tag does not belong to user', async () => {
      mockTagRepository.findBy.mockResolvedValue([otherTag]);

      await expect(
        service.create(mockUserId, {
          title: 'Test',
          content: 'Content',
          tagIds: [2],
        }),
      ).rejects.toThrow(ForbiddenException);
    });
  });

  describe('findAll', () => {
    beforeEach(() => {
      mockNoteRepository.createQueryBuilder.mockReturnValue(mockQueryBuilder);
    });

    it('should return all notes for user', async () => {
      mockQueryBuilder.getMany.mockResolvedValue([mockNoteWithBoard]);

      const result = await service.findAll(mockUserId, {});

      expect(result).toHaveLength(1);
      expect(mockQueryBuilder.where).toHaveBeenCalledWith('note.user_id = :userId', {
        userId: mockUserId,
      });
      expect(mockQueryBuilder.leftJoinAndSelect).toHaveBeenCalledWith('note.tags', 'tag');
      expect(mockQueryBuilder.orderBy).toHaveBeenCalledWith('note.updated_at', 'DESC');
      expect(mockQueryBuilder.getMany).toHaveBeenCalled();
    });

    it('should filter by search', async () => {
      mockQueryBuilder.getMany.mockResolvedValue([mockNoteWithBoard]);

      await service.findAll(mockUserId, { search: 'Board' });

      expect(mockQueryBuilder.andWhere).toHaveBeenCalledWith('note.title LIKE :search', {
        search: '%Board%',
      });
    });

    it('should filter by type BOARD', async () => {
      mockQueryBuilder.getMany.mockResolvedValue([mockNoteWithBoard]);

      await service.findAll(mockUserId, { type: NoteType.BOARD });

      expect(mockQueryBuilder.andWhere).toHaveBeenCalledWith('note.board_id IS NOT NULL');
    });

    it('should filter by type PROJECT', async () => {
      mockQueryBuilder.getMany.mockResolvedValue([mockNoteWithProject]);

      await service.findAll(mockUserId, { type: NoteType.PROJECT });

      expect(mockQueryBuilder.andWhere).toHaveBeenCalledWith('note.project_id IS NOT NULL');
    });

    it('should filter by type CARD', async () => {
      mockQueryBuilder.getMany.mockResolvedValue([mockNoteWithCard]);

      await service.findAll(mockUserId, { type: NoteType.CARD });

      expect(mockQueryBuilder.andWhere).toHaveBeenCalledWith('note.card_id IS NOT NULL');
    });

    it('should filter by type GENERAL', async () => {
      mockQueryBuilder.getMany.mockResolvedValue([mockNoteDirect]);

      await service.findAll(mockUserId, { type: NoteType.GENERAL });

      expect(mockQueryBuilder.andWhere).toHaveBeenCalledWith(
        'note.board_id IS NULL AND note.project_id IS NULL AND note.card_id IS NULL',
      );
    });

    it('should filter by tagId', async () => {
      mockQueryBuilder.getMany.mockResolvedValue([mockNoteWithBoard]);

      await service.findAll(mockUserId, { tagId: 1 });

      expect(mockQueryBuilder.andWhere).toHaveBeenCalledWith('tag.id = :tagId', { tagId: 1 });
    });

    it('should filter by boardId', async () => {
      mockQueryBuilder.getMany.mockResolvedValue([mockNoteWithBoard]);

      await service.findAll(mockUserId, { boardId: 1 });

      expect(mockQueryBuilder.andWhere).toHaveBeenCalledWith('note.board_id = :boardId', {
        boardId: 1,
      });
    });
  });

  describe('findById', () => {
    it('should return a note with board ownership', async () => {
      mockNoteRepository.findOne.mockResolvedValue(mockNoteWithBoard);

      const result = await service.findById(1, mockUserId);

      expect(result).toBeDefined();
      expect(result.id).toBe(1);
    });

    it('should return a note with project ownership', async () => {
      mockNoteRepository.findOne.mockResolvedValue(mockNoteWithProject);
      mockProjectRepository.findOne.mockResolvedValue(mockProject);

      const result = await service.findById(2, mockUserId);

      expect(result).toBeDefined();
      expect(result.id).toBe(2);
    });

    it('should return a note with card ownership', async () => {
      mockNoteRepository.findOne.mockResolvedValue(mockNoteWithCard);
      mockCardRepository.findOne.mockResolvedValue(mockCard);

      const result = await service.findById(3, mockUserId);

      expect(result).toBeDefined();
      expect(result.id).toBe(3);
    });

    it('should return a note with direct user_id ownership', async () => {
      mockNoteRepository.findOne.mockResolvedValue(mockNoteDirect);

      const result = await service.findById(4, mockUserId);

      expect(result).toBeDefined();
      expect(result.id).toBe(4);
    });

    it('should throw NotFoundException if note does not exist', async () => {
      mockNoteRepository.findOne.mockResolvedValue(null);

      await expect(service.findById(999, mockUserId)).rejects.toThrow(NotFoundException);
    });

    it('should throw ForbiddenException if board ownership fails', async () => {
      mockNoteRepository.findOne.mockResolvedValue({
        ...mockNoteWithBoard,
        board: { id: 1, user_id: 999 },
      });

      await expect(service.findById(1, mockUserId)).rejects.toThrow(ForbiddenException);
    });

    it('should throw ForbiddenException if project ownership fails', async () => {
      mockNoteRepository.findOne.mockResolvedValue(mockNoteWithProject);
      mockProjectRepository.findOne.mockResolvedValue({ id: 1, user_id: 999 });

      await expect(service.findById(2, mockUserId)).rejects.toThrow(ForbiddenException);
    });

    it('should throw ForbiddenException if card ownership fails', async () => {
      const otherColumn = {
        id: 1,
        board_id: 1,
        board: { id: 1, user_id: 999 },
      };
      mockNoteRepository.findOne.mockResolvedValue(mockNoteWithCard);
      mockCardRepository.findOne.mockResolvedValue({ id: 1, column_id: 1, column: otherColumn });

      await expect(service.findById(3, mockUserId)).rejects.toThrow(ForbiddenException);
    });

    it('should throw ForbiddenException if direct ownership fails', async () => {
      mockNoteRepository.findOne.mockResolvedValue({ ...mockNoteDirect, user_id: 999 });

      await expect(service.findById(4, mockUserId)).rejects.toThrow(ForbiddenException);
    });
  });

  describe('findByBoardId', () => {
    it('should return notes for a board', async () => {
      mockBoardRepository.findOne.mockResolvedValue(mockBoard);
      mockNoteRepository.find.mockResolvedValue([mockNoteWithBoard]);

      const result = await service.findByBoardId(1, mockUserId);

      expect(result).toHaveLength(1);
      expect(mockNoteRepository.find).toHaveBeenCalledWith({
        where: { board_id: 1 },
        relations: ['tags'],
        order: { updated_at: 'DESC' },
      });
    });

    it('should throw NotFoundException if board does not exist', async () => {
      mockBoardRepository.findOne.mockResolvedValue(null);

      await expect(service.findByBoardId(999, mockUserId)).rejects.toThrow(NotFoundException);
    });

    it('should throw ForbiddenException if board does not belong to user', async () => {
      mockBoardRepository.findOne.mockResolvedValue({ id: 1, user_id: 999 });

      await expect(service.findByBoardId(1, mockUserId)).rejects.toThrow(ForbiddenException);
    });
  });

  describe('update', () => {
    it('should update scalar fields', async () => {
      mockNoteRepository.findOne.mockResolvedValue(mockNoteWithBoard);
      mockNoteRepository.save.mockResolvedValue({ ...mockNoteWithBoard, title: 'Updated' });

      const result = await service.update(1, mockUserId, { title: 'Updated' });

      expect(result.title).toBe('Updated');
      expect(mockNoteRepository.save).toHaveBeenCalledWith(
        expect.objectContaining({ title: 'Updated' }),
      );
    });

    it('should update tags', async () => {
      const updatedTag = { id: 2, name: 'urgent', color: 'red', user_id: mockUserId } as Tag;
      mockTagRepository.findBy.mockResolvedValue([updatedTag]);
      mockNoteRepository.findOne.mockResolvedValue(mockNoteWithBoard);
      mockNoteRepository.save.mockResolvedValue({ ...mockNoteWithBoard, tags: [updatedTag] });

      const result = await service.update(1, mockUserId, { tagIds: [2] });

      expect(result.tags).toHaveLength(1);
      expect(result.tags[0].name).toBe('urgent');
    });

    it('should clear tags when tagIds is empty array', async () => {
      mockTagRepository.findBy.mockResolvedValue([]);
      mockNoteRepository.findOne.mockResolvedValue(mockNoteWithBoard);
      mockNoteRepository.save.mockResolvedValue({ ...mockNoteWithBoard, tags: [] });

      const result = await service.update(1, mockUserId, { tagIds: [] });

      expect(result.tags).toHaveLength(0);
    });

    it('should throw NotFoundException if note does not exist', async () => {
      mockNoteRepository.findOne.mockResolvedValue(null);

      await expect(service.update(999, mockUserId, { title: 'New' })).rejects.toThrow(
        NotFoundException,
      );
    });

    it('should throw ForbiddenException when updating tag not owned by user', async () => {
      mockNoteRepository.findOne.mockResolvedValueOnce(mockNoteWithBoard);
      mockTagRepository.findBy.mockResolvedValue([otherTag]);

      await expect(service.update(1, mockUserId, { tagIds: [2] })).rejects.toThrow(
        ForbiddenException,
      );
    });
  });

  describe('remove', () => {
    it('should delete a note', async () => {
      mockNoteRepository.findOne.mockResolvedValue(mockNoteWithBoard);
      mockNoteRepository.remove.mockResolvedValue(mockNoteWithBoard);

      await service.remove(1, mockUserId);

      expect(mockNoteRepository.remove).toHaveBeenCalledWith(mockNoteWithBoard);
    });

    it('should throw NotFoundException if note does not exist', async () => {
      mockNoteRepository.findOne.mockResolvedValue(null);

      await expect(service.remove(999, mockUserId)).rejects.toThrow(NotFoundException);
    });
  });
});
