import { Test, TestingModule } from '@nestjs/testing';
import { getRepositoryToken } from '@nestjs/typeorm';
import { ColumnsService } from './columns.service';
import { BoardColumn } from './entities/column.entity';
import { Board } from '../boards/entities/board.entity';
import { Card } from '../cards/entities/card.entity';
import { NotFoundException, ForbiddenException } from '@nestjs/common';

describe('ColumnsService', () => {
  let service: ColumnsService;

  type MockColumnFindOptions = {
    where?: { id?: number };
    relations?: string[];
  };

  type MockCardManager = {
    createQueryBuilder: jest.Mock;
    save: jest.Mock;
    transaction: jest.Mock;
  };

  const mockColumnRepository = {
    find: jest.fn(),
    findOne: jest.fn().mockImplementation((opts?: MockColumnFindOptions) => {
      if (!opts || !opts.where || !opts.where.id) return Promise.resolve(null);
      const id = opts.where.id;
      if (id === 1) {
        return Promise.resolve({
          id: 1,
          name: 'To Do',
          board: { user_id: 1 },
          cards: opts.relations?.includes('cards')
            ? [
                { id: 1, title: 'Card 1', column_id: 1, position: 0 },
                { id: 2, title: 'Card 2', column_id: 1, position: 1 },
              ]
            : [],
        });
      }
      if (id === 2) {
        return Promise.resolve({
          id: 2,
          name: 'In Progress',
          board: { user_id: 1 },
          cards: [],
        });
      }
      return Promise.resolve(null);
    }),
    create: jest.fn(),
    save: jest.fn(),
    remove: jest.fn(),
    createQueryBuilder: jest.fn(),
  };

  const mockBoardRepository = {
    findOne: jest.fn(),
  };

  const mockCardManager: MockCardManager = {
    createQueryBuilder: jest.fn(),
    save: jest.fn(),
    transaction: jest.fn(async (callback: (mgr: MockCardManager) => Promise<unknown>) =>
      callback(mockCardManager),
    ),
  };

  const mockCardRepository = {
    find: jest.fn(),
    findOne: jest.fn(),
    create: jest.fn(),
    save: jest.fn(),
    query: jest.fn(),
    createQueryBuilder: jest.fn(),
    manager: mockCardManager,
  };

  const mockQueryBuilderForCards = (cards: unknown[], flags: unknown[]) => {
    const qb: Record<string, jest.Mock> = {};
    qb.select = jest.fn().mockReturnValue(qb);
    qb.addSelect = jest.fn().mockReturnValue(qb);
    qb.leftJoinAndSelect = jest.fn().mockReturnValue(qb);
    qb.where = jest.fn().mockReturnValue(qb);
    qb.orderBy = jest.fn().mockReturnValue(qb);
    qb.getMany = jest.fn().mockResolvedValue(cards);
    qb.getRawMany = jest.fn().mockResolvedValue(flags);
    mockCardRepository.createQueryBuilder.mockImplementation(() => qb);
    return qb;
  };

  beforeEach(async () => {
    const module: TestingModule = await Test.createTestingModule({
      providers: [
        ColumnsService,
        {
          provide: getRepositoryToken(BoardColumn),
          useValue: mockColumnRepository,
        },
        {
          provide: getRepositoryToken(Board),
          useValue: mockBoardRepository,
        },
        {
          provide: getRepositoryToken(Card),
          useValue: mockCardRepository,
        },
      ],
    }).compile();

    service = module.get<ColumnsService>(ColumnsService);
  });

  afterEach(() => {
    jest.clearAllMocks();
  });

  describe('findAllByBoardId', () => {
    it('should return columns for a board without selecting description', async () => {
      const mockBoard = { id: 1, user_id: 1 };
      const mockColumns = [
        { id: 1, name: 'To Do', position: 0, board_id: 1 },
        { id: 2, name: 'In Progress', position: 1, board_id: 1 },
      ];

      mockBoardRepository.findOne.mockResolvedValue(mockBoard);
      mockColumnRepository.find.mockResolvedValue(mockColumns);
      const qb = mockQueryBuilderForCards([], []);

      const result = await service.findAllByBoardId(1, 1);

      expect(result).toHaveLength(2);
      expect(result[0].cards).toEqual([]);
      expect(mockBoardRepository.findOne).toHaveBeenCalledWith({
        where: { id: 1, user_id: 1 },
      });
      expect(mockColumnRepository.find).toHaveBeenCalledWith({
        where: { board_id: 1 },
        order: { position: 'ASC' },
      });
      // Main card query must not request description TEXT
      expect(qb.select).toHaveBeenCalledWith(
        expect.arrayContaining([
          'card.id',
          'card.title',
          'card.column_id',
          'card.position',
          'card.due_date',
          'card.created_at',
          'card.updated_at',
        ]),
      );
      const selectedCols: string[] = qb.select.mock.calls[0][0];
      expect(selectedCols.join(' ')).not.toContain('description');
    });

    it('should attach has_description flags without selecting description TEXT', async () => {
      const mockBoard = { id: 1, user_id: 1 };
      const mockColumns = [{ id: 1, name: 'To Do', position: 0, board_id: 1 }];
      const mockCards = [
        { id: 1, title: 'Card 1', column_id: 1, position: 0 },
        { id: 2, title: 'Card 2', column_id: 1, position: 1 },
      ];

      mockBoardRepository.findOne.mockResolvedValue(mockBoard);
      mockColumnRepository.find.mockResolvedValue(mockColumns);
      const qb = mockQueryBuilderForCards(mockCards, [
        { id: 1, has_description: 0 },
        { id: 2, has_description: 1 },
      ]);
      mockCardRepository.query.mockResolvedValueOnce([]);

      const result = await service.findAllByBoardId(1, 1);

      expect(result[0].cards).toHaveLength(2);
      expect(result[0].cards[0]).toMatchObject({ id: 1, has_description: false });
      expect(result[0].cards[1]).toMatchObject({ id: 2, has_description: true });
      for (const card of result[0].cards) {
        expect(card).not.toHaveProperty('description');
      }
      // Flag query computes has_description via SQL CASE, never selecting TEXT
      expect(qb.addSelect).toHaveBeenCalledWith(
        expect.stringContaining('CASE WHEN'),
        'has_description',
      );
      expect(qb.addSelect.mock.calls[0][0]).not.toContain('card.description,');
    });

    it('should attach checklist_progress to cards via batch aggregation', async () => {
      const mockBoard = { id: 1, user_id: 1 };
      const mockColumns = [
        {
          id: 1,
          name: 'To Do',
          position: 0,
          board_id: 1,
        },
      ];
      const mockCards = [{ id: 1, title: 'Card 1', column_id: 1, position: 0 }];

      mockBoardRepository.findOne.mockResolvedValue(mockBoard);
      mockColumnRepository.find.mockResolvedValue(mockColumns);
      mockQueryBuilderForCards(mockCards, [{ id: 1, has_description: 0 }]);
      mockCardRepository.query.mockResolvedValueOnce([{ card_id: 1, total: 3, completed: 1 }]);

      const result = await service.findAllByBoardId(1, 1);

      expect(result[0].cards[0]).toHaveProperty('checklist_progress');
      expect(result[0].cards[0].checklist_progress).toEqual({
        completed: 1,
        total: 3,
        percent: 33,
      });
    });

    it('should handle cards with zero checklists gracefully', async () => {
      const mockBoard = { id: 1, user_id: 1 };
      const mockColumns = [
        {
          id: 1,
          name: 'To Do',
          position: 0,
          board_id: 1,
        },
      ];
      const mockCards = [{ id: 1, title: 'Card 1', column_id: 1, position: 0 }];

      mockBoardRepository.findOne.mockResolvedValue(mockBoard);
      mockColumnRepository.find.mockResolvedValue(mockColumns);
      mockQueryBuilderForCards(mockCards, [{ id: 1, has_description: false }]);
      mockCardRepository.query.mockResolvedValueOnce([]);

      const result = await service.findAllByBoardId(1, 1);

      expect(result[0].cards[0].checklist_progress).toBeUndefined();
    });

    it('should handle cards with empty checklists (no items)', async () => {
      const mockBoard = { id: 1, user_id: 1 };
      const mockColumns = [
        {
          id: 1,
          name: 'To Do',
          position: 0,
          board_id: 1,
        },
      ];
      const mockCards = [{ id: 1, title: 'Card 1', column_id: 1, position: 0 }];

      mockBoardRepository.findOne.mockResolvedValue(mockBoard);
      mockColumnRepository.find.mockResolvedValue(mockColumns);
      mockQueryBuilderForCards(mockCards, [{ id: 1, has_description: 1 }]);
      mockCardRepository.query.mockResolvedValueOnce([{ card_id: 1, total: 0, completed: 0 }]);

      const result = await service.findAllByBoardId(1, 1);

      expect(result[0].cards[0].checklist_progress).toEqual({
        completed: 0,
        total: 0,
        percent: 0,
      });
    });

    it('should throw NotFoundException when board not found', async () => {
      mockBoardRepository.findOne.mockResolvedValue(null);

      await expect(service.findAllByBoardId(1, 1)).rejects.toThrow(NotFoundException);
    });
  });

  describe('create', () => {
    it('should create a new column', async () => {
      const mockBoard = { id: 1, user_id: 1 };
      const mockColumn = {
        id: 1,
        name: 'New Column',
        position: 0,
        board_id: 1,
      };

      mockBoardRepository.findOne.mockResolvedValue(mockBoard);
      mockColumnRepository.createQueryBuilder.mockReturnValue({
        where: jest.fn().mockReturnThis(),
        select: jest.fn().mockReturnThis(),
        getRawOne: jest.fn().mockResolvedValue({ max: -1 }),
      });
      mockColumnRepository.create.mockReturnValue(mockColumn);
      mockColumnRepository.save.mockResolvedValue(mockColumn);

      const result = await service.create(1, 1, { name: 'New Column' });

      expect(result).toEqual(mockColumn);
      expect(mockColumnRepository.create).toHaveBeenCalledWith({
        name: 'New Column',
        position: 0,
        board_id: 1,
      });
    });

    it('should use default name when not provided', async () => {
      const mockBoard = { id: 1, user_id: 1 };
      const mockColumn = {
        id: 1,
        name: 'New Column',
        position: 0,
        board_id: 1,
      };

      mockBoardRepository.findOne.mockResolvedValue(mockBoard);
      mockColumnRepository.createQueryBuilder.mockReturnValue({
        where: jest.fn().mockReturnThis(),
        select: jest.fn().mockReturnThis(),
        getRawOne: jest.fn().mockResolvedValue({ max: -1 }),
      });
      mockColumnRepository.create.mockReturnValue(mockColumn);
      mockColumnRepository.save.mockResolvedValue(mockColumn);

      await service.create(1, 1, {});

      expect(mockColumnRepository.create).toHaveBeenCalledWith({
        name: 'New Column',
        position: 0,
        board_id: 1,
      });
    });

    it('should throw NotFoundException when board not found', async () => {
      mockBoardRepository.findOne.mockResolvedValue(null);

      await expect(service.create(1, 1, { name: 'Test' })).rejects.toThrow(NotFoundException);
    });
  });

  describe('update', () => {
    it('should update column name', async () => {
      const mockColumn = {
        id: 1,
        name: 'To Do',
        board: { user_id: 1 },
      };
      const updatedColumn = { ...mockColumn, name: 'In Progress' };

      mockColumnRepository.findOne.mockResolvedValue(mockColumn);
      mockColumnRepository.save.mockResolvedValue(updatedColumn);

      const result = await service.update(1, 1, { name: 'In Progress' });

      expect(result.name).toBe('In Progress');
    });

    it('should throw NotFoundException when column not found', async () => {
      mockColumnRepository.findOne.mockResolvedValue(null);

      await expect(service.update(1, 1, { name: 'Test' })).rejects.toThrow(NotFoundException);
    });

    it('should throw ForbiddenException when user does not own board', async () => {
      const mockColumn = {
        id: 1,
        name: 'To Do',
        board: { user_id: 2 },
      };

      mockColumnRepository.findOne.mockResolvedValue(mockColumn);

      await expect(service.update(1, 1, { name: 'Test' })).rejects.toThrow(ForbiddenException);
    });
  });

  describe('remove', () => {
    it('should delete a column', async () => {
      const mockColumn = {
        id: 1,
        name: 'To Do',
        board: { user_id: 1 },
      };

      mockColumnRepository.findOne.mockResolvedValue(mockColumn);
      mockColumnRepository.remove.mockResolvedValue(mockColumn);

      await service.remove(1, 1);

      expect(mockColumnRepository.remove).toHaveBeenCalledWith(mockColumn);
    });

    it('should throw NotFoundException when column not found', async () => {
      mockColumnRepository.findOne.mockResolvedValue(null);

      await expect(service.remove(1, 1)).rejects.toThrow(NotFoundException);
    });
  });

  describe('sortCards', () => {
    it('should sort cards by created_at ascending without selecting description', async () => {
      const mockColumn = {
        id: 1,
        name: 'To Do',
        board: { user_id: 1 },
      };
      const cardsForSort = [
        { id: 1, title: 'Card 1', created_at: new Date('2026-01-01'), position: 0 },
        { id: 2, title: 'Card 2', created_at: new Date('2026-01-02'), position: 1 },
      ];
      const summaryCards = [
        { id: 2, title: 'Card 2', column_id: 1, position: 0 },
        { id: 1, title: 'Card 1', column_id: 1, position: 1 },
      ];

      mockColumnRepository.findOne
        .mockResolvedValueOnce(mockColumn)
        .mockResolvedValueOnce({ ...mockColumn });
      mockCardRepository.find.mockResolvedValue(cardsForSort);
      mockCardRepository.save.mockResolvedValue([]);
      mockQueryBuilderForCards(summaryCards, [
        { id: 1, has_description: 0 },
        { id: 2, has_description: 1 },
      ]);
      mockCardRepository.query.mockResolvedValueOnce([]);

      const result = await service.sortCards(1, 1, 'asc');

      expect(mockCardRepository.find).toHaveBeenCalledWith({
        where: { column_id: 1 },
        order: { created_at: 'ASC' },
      });
      expect(mockCardRepository.save).toHaveBeenCalledTimes(1);
      expect(result.cards).toHaveLength(2);
      for (const card of result.cards) {
        expect(card).not.toHaveProperty('description');
      }
    });

    it('should sort cards by created_at descending', async () => {
      const mockColumn = {
        id: 1,
        name: 'To Do',
        board: { user_id: 1 },
      };
      const cardsForSort = [
        { id: 1, title: 'Card 1', created_at: new Date('2026-01-01'), position: 0 },
        { id: 2, title: 'Card 2', created_at: new Date('2026-01-02'), position: 1 },
      ];
      const summaryCards = [
        { id: 1, title: 'Card 1', column_id: 1, position: 0 },
        { id: 2, title: 'Card 2', column_id: 1, position: 1 },
      ];

      mockColumnRepository.findOne
        .mockResolvedValueOnce(mockColumn)
        .mockResolvedValueOnce({ ...mockColumn });
      mockCardRepository.find.mockResolvedValue(cardsForSort);
      mockCardRepository.save.mockResolvedValue([]);
      mockQueryBuilderForCards(summaryCards, [
        { id: 1, has_description: 0 },
        { id: 2, has_description: 0 },
      ]);
      mockCardRepository.query.mockResolvedValueOnce([]);

      await service.sortCards(1, 1, 'desc');

      expect(mockCardRepository.find).toHaveBeenCalledWith({
        where: { column_id: 1 },
        order: { created_at: 'DESC' },
      });
    });

    it('should throw NotFoundException when column not found', async () => {
      mockColumnRepository.findOne.mockResolvedValue(null);

      await expect(service.sortCards(1, 1, 'asc')).rejects.toThrow(NotFoundException);
    });

    it('should throw ForbiddenException when user does not own board', async () => {
      const mockColumn = {
        id: 1,
        name: 'To Do',
        board: { user_id: 2 },
      };

      mockColumnRepository.findOne.mockResolvedValue(mockColumn);

      await expect(service.sortCards(1, 1, 'asc')).rejects.toThrow(ForbiddenException);
    });
  });

  describe('moveAllCards', () => {
    it('should move all cards to target column', async () => {
      const sourceColumn = {
        id: 1,
        name: 'To Do',
        board: { user_id: 1 },
        cards: [
          { id: 1, title: 'Card 1', column_id: 1, position: 0 },
          { id: 2, title: 'Card 2', column_id: 1, position: 1 },
        ],
      };
      const targetColumn = {
        id: 2,
        name: 'In Progress',
        board: { user_id: 1 },
        cards: [],
      };

      mockColumnRepository.findOne
        .mockResolvedValueOnce(sourceColumn)
        .mockResolvedValueOnce(targetColumn);
      mockCardRepository.find.mockResolvedValue(sourceColumn.cards);
      mockCardManager.createQueryBuilder.mockReturnValue({
        where: jest.fn().mockReturnThis(),
        select: jest.fn().mockReturnThis(),
        getRawOne: jest.fn().mockResolvedValue({ max: 0 }),
      });
      mockCardManager.save.mockResolvedValue([]);

      const result = await service.moveAllCards(1, 2, 1);

      expect(result.movedCount).toBe(2);
      expect(result.targetName).toBe('In Progress');
      expect(mockCardManager.save).toHaveBeenCalledTimes(1);
    });

    it('should throw BadRequestException when source and target are the same', async () => {
      const mockColumn = {
        id: 1,
        name: 'To Do',
        board: { user_id: 1 },
        cards: [],
      };

      mockColumnRepository.findOne.mockResolvedValue(mockColumn);

      await expect(service.moveAllCards(1, 1, 1)).rejects.toThrow('Cards already in this column');
    });

    it('should throw BadRequestException when no cards to move', async () => {
      const sourceColumn = {
        id: 1,
        name: 'To Do',
        board: { user_id: 1 },
        cards: [],
      };
      const targetColumn = {
        id: 2,
        name: 'In Progress',
        board: { user_id: 1 },
        cards: [],
      };

      mockColumnRepository.findOne
        .mockResolvedValueOnce(sourceColumn)
        .mockResolvedValueOnce(targetColumn);
      mockCardRepository.find.mockResolvedValue([]);

      await expect(service.moveAllCards(1, 2, 1)).rejects.toThrow('No cards to move');
    });

    it('should throw NotFoundException when source column not found', async () => {
      mockColumnRepository.findOne.mockResolvedValue(null);

      await expect(service.moveAllCards(1, 2, 1)).rejects.toThrow(NotFoundException);
    });

    it('should throw ForbiddenException when user does not own target board', async () => {
      const sourceColumn = {
        id: 1,
        name: 'To Do',
        board: { user_id: 1 },
        cards: [],
      };
      const targetColumn = {
        id: 2,
        name: 'In Progress',
        board: { user_id: 2 },
        cards: [],
      };

      mockColumnRepository.findOne
        .mockResolvedValueOnce(sourceColumn)
        .mockResolvedValueOnce(targetColumn);

      await expect(service.moveAllCards(1, 2, 1)).rejects.toThrow(ForbiddenException);
    });
  });
});
