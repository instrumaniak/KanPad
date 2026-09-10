import { Test, TestingModule } from '@nestjs/testing';
import { getRepositoryToken } from '@nestjs/typeorm';
import { NotFoundException, ForbiddenException } from '@nestjs/common';
import { BoardsService } from './boards.service';
import { Board } from './entities/board.entity';
import { BoardColumn } from '../columns/entities/column.entity';

describe('BoardsService', () => {
  let service: BoardsService;

  const mockBoardRepository = {
    find: jest.fn(),
    findOne: jest.fn(),
    create: jest.fn(),
    save: jest.fn(),
    remove: jest.fn(),
    update: jest.fn(),
  };

  const mockColumnRepository = {
    create: jest.fn(),
    save: jest.fn(),
  };

  beforeEach(async () => {
    const module: TestingModule = await Test.createTestingModule({
      providers: [
        BoardsService,
        { provide: getRepositoryToken(Board), useValue: mockBoardRepository },
        { provide: getRepositoryToken(BoardColumn), useValue: mockColumnRepository },
      ],
    }).compile();

    service = module.get<BoardsService>(BoardsService);
  });

  afterEach(() => {
    jest.clearAllMocks();
  });

  describe('findAllByUserId', () => {
    it('should return all boards for a user', async () => {
      const boards = [
        { id: 1, name: 'Board 1', user_id: 1, project_id: null },
        { id: 2, name: 'Board 2', user_id: 1, project_id: 1 },
      ];
      mockBoardRepository.find.mockResolvedValue(boards as Board[]);

      const result = await service.findAllByUserId(1);

      expect(result).toEqual(boards);
      expect(mockBoardRepository.find).toHaveBeenCalledWith({
        where: { user_id: 1, is_archived: false },
        relations: ['project'],
        order: { updated_at: 'DESC' },
      });
    });

    it('should filter by projectId when provided', async () => {
      const boards = [{ id: 1, name: 'Board 1', user_id: 1, project_id: 1 }];
      mockBoardRepository.find.mockResolvedValue(boards as Board[]);

      const result = await service.findAllByUserId(1, 1);

      expect(result).toEqual(boards);
      expect(mockBoardRepository.find).toHaveBeenCalledWith({
        where: { user_id: 1, project_id: 1, is_archived: false },
        relations: ['project'],
        order: { updated_at: 'DESC' },
      });
    });
  });

  describe('findOne', () => {
    it('should return a board if found', async () => {
      const board = {
        id: 1,
        name: 'Board 1',
        user_id: 1,
        columns: [],
      };
      mockBoardRepository.findOne.mockResolvedValue(board as unknown as Board);

      const result = await service.findOne(1, 1);

      expect(result).toEqual(board);
    });

    it('should throw NotFoundException if board not found', async () => {
      mockBoardRepository.findOne.mockResolvedValue(null);

      await expect(service.findOne(999, 1)).rejects.toThrow(NotFoundException);
    });
  });

  describe('create', () => {
    it('should create a board with default columns', async () => {
      const createDto = { name: 'New Board', background_color: '#0079BF', project_id: null };
      const savedBoard = { id: 1, ...createDto, user_id: 1, columns: [] };
      const createdBoard = { id: 1, ...createDto, user_id: 1 };

      mockBoardRepository.create.mockReturnValue(createdBoard as unknown as Board);
      mockBoardRepository.save.mockResolvedValue(savedBoard as unknown as Board);
      mockBoardRepository.findOne.mockResolvedValue(savedBoard as unknown as Board);
      mockColumnRepository.create.mockReturnValue([
        { name: 'To Do', position: 0, board_id: 1 },
        { name: 'In Progress', position: 1, board_id: 1 },
        { name: 'Done', position: 2, board_id: 1 },
      ] as BoardColumn[]);
      mockColumnRepository.save.mockResolvedValue([]);

      const result = await service.create(1, createDto);

      expect(result.name).toBe('New Board');
      expect(mockColumnRepository.save).toHaveBeenCalled();
    });

    it('should use default color if not provided', async () => {
      const createDto = { name: 'New Board' };
      const createdBoard = { id: 1, name: 'New Board', background_color: '#0079BF', user_id: 1 };

      mockBoardRepository.create.mockReturnValue(createdBoard as unknown as Board);
      mockBoardRepository.save.mockResolvedValue(createdBoard as unknown as Board);
      mockBoardRepository.findOne.mockResolvedValue(createdBoard as unknown as Board);
      mockColumnRepository.create.mockReturnValue([] as BoardColumn[]);
      mockColumnRepository.save.mockResolvedValue([]);

      const result = await service.create(1, createDto);

      expect(result.background_color).toBe('#0079BF');
    });
  });

  describe('update', () => {
    it('should update board name', async () => {
      const board = { id: 1, name: 'Old Name', background_color: '#0079BF', user_id: 1 };
      const updatedBoard = { ...board, name: 'New Name' };

      mockBoardRepository.findOne.mockResolvedValue(board as Board);
      mockBoardRepository.save.mockResolvedValue(updatedBoard as Board);

      const result = await service.update(1, 1, { name: 'New Name' });

      expect(result.name).toBe('New Name');
    });

    it('should update board color', async () => {
      const board = { id: 1, name: 'Board', background_color: '#0079BF', user_id: 1 };
      const updatedBoard = { ...board, background_color: '#FFAB00' };

      mockBoardRepository.findOne.mockResolvedValue(board as Board);
      mockBoardRepository.save.mockResolvedValue(updatedBoard as Board);

      const result = await service.update(1, 1, { background_color: '#FFAB00' });

      expect(result.background_color).toBe('#FFAB00');
    });

    it('should throw ForbiddenException if project not found or access denied', async () => {
      const board = { id: 1, name: 'Board', background_color: '#0079BF', user_id: 1 };
      const mockManager = {
        findOne: jest.fn().mockResolvedValue(null),
      };
      const repositoryWithManager = mockBoardRepository as typeof mockBoardRepository & {
        manager: typeof mockManager;
      };
      repositoryWithManager.manager = mockManager;

      mockBoardRepository.findOne.mockResolvedValue(board as Board);

      await expect(service.update(1, 1, { project_id: 999 })).rejects.toThrow(ForbiddenException);
    });

    it('should update view_mode from board to list and persist', async () => {
      const board = {
        id: 1,
        name: 'Board',
        background_color: '#0079BF',
        user_id: 1,
        view_mode: 'board',
      };
      const updatedBoard = { ...board, view_mode: 'list' };

      mockBoardRepository.findOne.mockResolvedValue(board as unknown as Board);
      mockBoardRepository.save.mockResolvedValue(updatedBoard as unknown as Board);

      const result = await service.update(1, 1, { view_mode: 'list' });

      expect(result.view_mode).toBe('list');
      expect(mockBoardRepository.save).toHaveBeenCalledWith(
        expect.objectContaining({ view_mode: 'list' }),
      );

      // Verify persistence via follow-up findOne DB query (not just response)
      mockBoardRepository.findOne.mockResolvedValue(updatedBoard as unknown as Board);
      const persisted = await service.findOne(1, 1);
      expect(persisted.view_mode).toBe('list');
    });

    it('should default to board view_mode on create', async () => {
      const createDto = { name: 'New Board' };
      const createdBoard = {
        id: 1,
        name: 'New Board',
        background_color: '#0079BF',
        user_id: 1,
        view_mode: 'board',
      };

      mockBoardRepository.create.mockReturnValue(createdBoard as unknown as Board);
      mockBoardRepository.save.mockResolvedValue(createdBoard as unknown as Board);
      mockBoardRepository.findOne.mockResolvedValue(createdBoard as unknown as Board);

      const result = await service.create(1, createDto);

      expect(result.view_mode).toBe('board');
    });

    it('should ignore null/invalid view_mode instead of persisting it', async () => {
      const board = {
        id: 1,
        name: 'Board',
        background_color: '#0079BF',
        user_id: 1,
        view_mode: 'board',
      };

      mockBoardRepository.findOne.mockResolvedValue(board as unknown as Board);
      mockBoardRepository.save.mockImplementation((b: unknown) => Promise.resolve(b as Board));

      const nullResult = await service.update(1, 1, { view_mode: null as unknown as 'list' });
      expect(nullResult.view_mode).toBe('board');

      const invalidResult = await service.update(1, 1, {
        view_mode: 'grid' as unknown as 'list',
      });
      expect(invalidResult.view_mode).toBe('board');
    });
  });

  describe('remove', () => {
    it('should delete a board', async () => {
      const board = { id: 1, name: 'Board', user_id: 1, background_color: '#0079BF' };
      mockBoardRepository.findOne.mockResolvedValue(board as Board);
      mockBoardRepository.remove.mockResolvedValue(board as Board);

      await service.remove(1, 1);

      expect(mockBoardRepository.remove).toHaveBeenCalled();
      expect(mockBoardRepository.remove).toHaveBeenCalledWith(
        expect.objectContaining({ id: 1, name: 'Board' }),
      );
    });

    it('should throw NotFoundException if board not found', async () => {
      mockBoardRepository.findOne.mockResolvedValue(null);

      await expect(service.remove(999, 1)).rejects.toThrow(NotFoundException);
    });
  });

  describe('findAllByUserId', () => {
    it('should not return archived boards by default', async () => {
      const boards = [
        { id: 1, name: 'Board 1', user_id: 1, is_archived: false },
        { id: 2, name: 'Board 2', user_id: 1, is_archived: false },
      ];
      mockBoardRepository.find.mockResolvedValue(boards as Board[]);

      const result = await service.findAllByUserId(1);

      expect(result).toEqual(boards);
      expect(mockBoardRepository.find).toHaveBeenCalledWith({
        where: { user_id: 1, is_archived: false },
        relations: ['project'],
        order: { updated_at: 'DESC' },
      });
    });

    it('should return all boards when includeArchived is true', async () => {
      const boards = [
        { id: 1, name: 'Board 1', user_id: 1, is_archived: false },
        { id: 2, name: 'Board 2', user_id: 1, is_archived: true },
      ];
      mockBoardRepository.find.mockResolvedValue(boards as Board[]);

      const result = await service.findAllByUserId(1, undefined, true);

      expect(result).toEqual(boards);
      expect(mockBoardRepository.find).toHaveBeenCalledWith({
        where: { user_id: 1 },
        relations: ['project'],
        order: { updated_at: 'DESC' },
      });
    });
  });

  describe('findAllArchivedByUserId', () => {
    it('should return only archived boards', async () => {
      const boards = [{ id: 2, name: 'Archived Board', user_id: 1, is_archived: true }];
      mockBoardRepository.find.mockResolvedValue(boards as Board[]);

      const result = await service.findAllArchivedByUserId(1);

      expect(result).toEqual(boards);
      expect(mockBoardRepository.find).toHaveBeenCalledWith({
        where: { user_id: 1, is_archived: true },
        relations: ['project'],
        order: { updated_at: 'DESC' },
      });
    });
  });

  describe('archive', () => {
    it('should archive a board', async () => {
      const board = { id: 1, name: 'Board', user_id: 1, is_archived: false };
      const archivedBoard = { ...board, is_archived: true };
      mockBoardRepository.findOne.mockResolvedValue(board as Board);
      mockBoardRepository.save.mockResolvedValue(archivedBoard as Board);

      const result = await service.archive(1, 1);

      expect(result.is_archived).toBe(true);
      expect(mockBoardRepository.save).toHaveBeenCalledWith(
        expect.objectContaining({ id: 1, is_archived: true }),
      );
    });

    it('should throw NotFoundException if board not found', async () => {
      mockBoardRepository.findOne.mockResolvedValue(null);

      await expect(service.archive(999, 1)).rejects.toThrow(NotFoundException);
    });
  });

  describe('restore', () => {
    it('should restore an archived board', async () => {
      const board = { id: 1, name: 'Board', user_id: 1, is_archived: true };
      const restoredBoard = { ...board, is_archived: false };
      mockBoardRepository.findOne.mockResolvedValue(board as Board);
      mockBoardRepository.save.mockResolvedValue(restoredBoard as Board);

      const result = await service.restore(1, 1);

      expect(result.is_archived).toBe(false);
      expect(mockBoardRepository.save).toHaveBeenCalledWith(
        expect.objectContaining({ id: 1, is_archived: false }),
      );
    });

    it('should throw NotFoundException if board not found', async () => {
      mockBoardRepository.findOne.mockResolvedValue(null);

      await expect(service.restore(999, 1)).rejects.toThrow(NotFoundException);
    });
  });

  describe('permanentDelete', () => {
    it('should permanently delete an archived board', async () => {
      const board = { id: 1, name: 'Board', user_id: 1, is_archived: true };
      mockBoardRepository.findOne.mockResolvedValue(board as Board);
      mockBoardRepository.remove.mockResolvedValue(board as Board);

      await service.permanentDelete(1, 1);

      expect(mockBoardRepository.remove).toHaveBeenCalledWith(expect.objectContaining({ id: 1 }));
    });

    it('should throw NotFoundException if board not found', async () => {
      mockBoardRepository.findOne.mockResolvedValue(null);

      await expect(service.permanentDelete(999, 1)).rejects.toThrow(NotFoundException);
    });

    it('should throw ForbiddenException if board is not archived', async () => {
      const board = { id: 1, name: 'Board', user_id: 1, is_archived: false };
      mockBoardRepository.findOne.mockResolvedValue(board as Board);

      await expect(service.permanentDelete(1, 1)).rejects.toThrow(ForbiddenException);
    });
  });
});
