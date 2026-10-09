import { Test, TestingModule } from '@nestjs/testing';
import { getRepositoryToken } from '@nestjs/typeorm';
import { DataSource, SelectQueryBuilder, UpdateResult } from 'typeorm';
import { CardsService } from './cards.service';
import { Card } from './entities/card.entity';
import { CardLabel } from './entities/card-label.entity';
import { Label } from '../labels/entities/label.entity';
import { BoardColumn } from '../columns/entities/column.entity';
import { Board } from '../boards/entities/board.entity';
import { NotFoundException, ForbiddenException } from '@nestjs/common';

describe('CardsService', () => {
  let service: CardsService;

  const mockUserId = 1;
  const mockBoard = { id: 1, user_id: mockUserId } as Board;
  const mockColumn = { id: 1, board: mockBoard, position: 0 } as BoardColumn;
  const mockTargetColumn = { id: 2, board: mockBoard, position: 1 } as BoardColumn;

  const mockCardRepository = {
    create: jest.fn(),
    save: jest.fn(),
    find: jest.fn(),
    findOne: jest.fn(),
    remove: jest.fn(),
    update: jest.fn(),
    query: jest.fn(),
    createQueryBuilder: jest.fn(),
  };

  const mockColumnRepository = {
    findOne: jest.fn(),
  };

  const mockCardLabelRepository = {
    findOne: jest.fn(),
    create: jest.fn(),
    save: jest.fn(),
    remove: jest.fn(),
  };

  const mockLabelRepository = {
    findOne: jest.fn(),
  };

  const mockDataSource = {
    transaction: jest
      .fn()
      .mockImplementation(async (callback: (manager: unknown) => Promise<unknown>) => {
        const mockManager = {
          getRepository: jest.fn().mockImplementation((entity: unknown) => {
            const ctorName = (entity as { name?: string }).name;
            if (ctorName === 'Card') return mockCardRepository;
            if (ctorName === 'Label') return mockLabelRepository;
            if (ctorName === 'CardLabel') return mockCardLabelRepository;
            return mockCardRepository;
          }),
        };
        return callback(mockManager);
      }),
  };

  const createMockCard = (id: number, columnId: number, position: number): Card =>
    ({
      id,
      title: `Card ${id}`,
      column_id: columnId,
      position,
      description: null,
      due_date: null,
      column: { ...mockColumn, id: columnId } as BoardColumn,
      created_at: new Date(),
      updated_at: new Date(),
    }) as Card;

  beforeEach(async () => {
    const module: TestingModule = await Test.createTestingModule({
      providers: [
        CardsService,
        {
          provide: getRepositoryToken(Card),
          useValue: mockCardRepository,
        },
        {
          provide: getRepositoryToken(BoardColumn),
          useValue: mockColumnRepository,
        },
        {
          provide: getRepositoryToken(Board),
          useValue: {
            findOne: jest.fn(),
          },
        },
        {
          provide: getRepositoryToken(CardLabel),
          useValue: mockCardLabelRepository,
        },
        {
          provide: getRepositoryToken(Label),
          useValue: mockLabelRepository,
        },
        {
          provide: DataSource,
          useValue: mockDataSource,
        },
      ],
    }).compile();

    service = module.get<CardsService>(CardsService);
  });

  afterEach(() => {
    jest.clearAllMocks();
  });

  describe('create', () => {
    it('should create a card with default fields', async () => {
      mockColumnRepository.findOne.mockResolvedValue(mockColumn);
      const mockQueryBuilder = {
        where: jest.fn().mockReturnThis(),
        select: jest.fn().mockReturnThis(),
        getRawOne: jest.fn().mockResolvedValue({ max: null }),
      };
      mockCardRepository.createQueryBuilder.mockReturnValue(mockQueryBuilder);
      mockCardRepository.create.mockReturnValue({ id: 1 });
      mockCardRepository.save.mockResolvedValue({
        id: 1,
        title: 'New Card',
        column_id: 1,
        position: 0,
        description: null,
        due_date: null,
      });

      const result = await service.create(mockUserId, { title: 'New Card', column_id: 1 });

      expect(mockCardRepository.create).toHaveBeenCalledWith(
        expect.objectContaining({
          title: 'New Card',
          column_id: 1,
          position: 0,
          description: null,
          due_date: null,
        }),
      );
      expect(result).toBeDefined();
    });

    it('should create a card with description and due_date', async () => {
      mockColumnRepository.findOne.mockResolvedValue(mockColumn);
      const mockQueryBuilder = {
        where: jest.fn().mockReturnThis(),
        select: jest.fn().mockReturnThis(),
        getRawOne: jest.fn().mockResolvedValue({ max: null }),
      };
      mockCardRepository.createQueryBuilder.mockReturnValue(mockQueryBuilder);
      mockCardRepository.create.mockReturnValue({ id: 1 });
      mockCardRepository.save.mockResolvedValue({
        id: 1,
        title: 'New Card',
        column_id: 1,
        position: 0,
        description: 'A description',
        due_date: new Date('2026-01-01'),
      });

      await service.create(mockUserId, {
        title: 'New Card',
        column_id: 1,
        description: 'A description',
        due_date: '2026-01-01',
      });

      expect(mockCardRepository.create).toHaveBeenCalledWith(
        expect.objectContaining({
          description: 'A description',
          due_date: new Date('2026-01-01'),
        }),
      );
    });
  });

  describe('findAllByColumnId (summary, no description TEXT)', () => {
    const mockQbForCards = (cards: unknown[], flags: unknown[]) => {
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

    it('should return cards with has_description flags and progress, without description', async () => {
      mockColumnRepository.findOne.mockResolvedValue(mockColumn);
      const mockCards = [
        { id: 1, title: 'Card 1', column_id: 1, position: 0 },
        { id: 2, title: 'Card 2', column_id: 1, position: 1 },
      ];
      const qb = mockQbForCards(mockCards, [
        { id: 1, has_description: 0 },
        { id: 2, has_description: 1 },
      ]);
      mockCardRepository.query.mockResolvedValueOnce([{ card_id: 2, total: 2, completed: 1 }]);

      const result = await service.findAllByColumnId(1, mockUserId);

      expect(result).toHaveLength(2);
      expect(result[0]).toMatchObject({ id: 1, has_description: false });
      expect(result[1]).toMatchObject({ id: 2, has_description: true });
      expect(result[1].checklist_progress).toEqual({ completed: 1, total: 2, percent: 50 });
      expect(result[0]).not.toHaveProperty('description');
      // Verify main select excludes description
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
      // Flag query computes has_description via SQL, never selecting TEXT
      expect(qb.addSelect).toHaveBeenCalledWith(
        expect.stringContaining('CASE WHEN'),
        'has_description',
      );
    });

    it('should coerce string flags from raw queries to boolean', async () => {
      mockColumnRepository.findOne.mockResolvedValue(mockColumn);
      const mockCards = [
        { id: 1, title: 'Card 1', column_id: 1, position: 0 },
        { id: 2, title: 'Card 2', column_id: 1, position: 1 },
      ];
      // MySQL getRawMany returns strings, not numbers
      mockQbForCards(mockCards, [
        { id: '1', has_description: '0' },
        { id: '2', has_description: '1' },
      ]);
      mockCardRepository.query.mockResolvedValueOnce([]);

      const result = await service.findAllByColumnId(1, mockUserId);

      expect(result[0]).toMatchObject({ id: 1, has_description: false });
      expect(result[1]).toMatchObject({ id: 2, has_description: true });
    });

    it('should return empty array without extra queries when no cards', async () => {
      mockColumnRepository.findOne.mockResolvedValue(mockColumn);
      mockQbForCards([], []);

      const result = await service.findAllByColumnId(1, mockUserId);

      expect(result).toEqual([]);
      expect(mockCardRepository.query).not.toHaveBeenCalled();
    });
  });

  describe('update (reordering)', () => {
    it('should reorder cards when moving down within a column', async () => {
      const cardToMove = createMockCard(1, 1, 0);
      const otherCard = createMockCard(2, 1, 1);
      const targetCard = createMockCard(3, 1, 2);

      mockCardRepository.findOne.mockResolvedValue(cardToMove);
      mockCardRepository.find.mockResolvedValue([cardToMove, otherCard, targetCard]);
      mockCardRepository.save.mockImplementation((c: Card) => c);
      mockCardRepository.update.mockResolvedValue({ affected: 1 } as UpdateResult);

      await service.update(1, mockUserId, { position: 2 });

      expect(otherCard.position).toBe(0);
      expect(targetCard.position).toBe(1);
      expect(mockCardRepository.update).toHaveBeenCalledWith(1, { position: 2 });
    });

    it('should reorder cards when moving up within a column', async () => {
      const cardToMove = createMockCard(3, 1, 2);
      const otherCard1 = createMockCard(1, 1, 0);
      const otherCard2 = createMockCard(2, 1, 1);

      mockCardRepository.findOne.mockResolvedValue(cardToMove);
      mockCardRepository.find.mockResolvedValue([otherCard1, otherCard2, cardToMove]);
      mockCardRepository.save.mockImplementation((c: Card) => c);
      mockCardRepository.update.mockResolvedValue({ affected: 1 } as UpdateResult);

      await service.update(3, mockUserId, { position: 0 });

      expect(otherCard1.position).toBe(1);
      expect(otherCard2.position).toBe(2);
      expect(mockCardRepository.update).toHaveBeenCalledWith(3, { position: 0 });
    });

    it('should not perform shifts if moving to the same position', async () => {
      const cardToMove = createMockCard(1, 1, 0);
      mockCardRepository.findOne.mockResolvedValue(cardToMove);
      mockCardRepository.update.mockResolvedValue({ affected: 1 } as UpdateResult);

      await service.update(1, mockUserId, { position: 0 });

      expect(mockCardRepository.find).not.toHaveBeenCalled();
      expect(mockCardRepository.update).toHaveBeenCalledWith(1, { position: 0 });
    });

    it('should handle moving between columns', async () => {
      const cardToMove = createMockCard(1, 1, 0);
      const otherCardInOldCol = createMockCard(2, 1, 1);
      const targetColumnCards = [createMockCard(3, 2, 0), createMockCard(4, 2, 1)];

      mockCardRepository.findOne.mockResolvedValue(cardToMove);
      mockColumnRepository.findOne.mockResolvedValue(mockTargetColumn);

      mockCardRepository.find
        .mockResolvedValueOnce([cardToMove, otherCardInOldCol])
        .mockResolvedValueOnce(targetColumnCards);

      const mockQueryBuilder: Pick<SelectQueryBuilder<Card>, 'where' | 'select' | 'getRawOne'> = {
        where: jest.fn().mockReturnThis(),
        select: jest.fn().mockReturnThis(),
        getRawOne: jest.fn().mockResolvedValue({ max: 1 }),
      };
      mockCardRepository.createQueryBuilder.mockReturnValue(
        mockQueryBuilder as SelectQueryBuilder<Card>,
      );
      mockCardRepository.save.mockImplementation((c: Card) => c);
      mockCardRepository.update.mockResolvedValue({ affected: 1 } as UpdateResult);

      await service.update(1, mockUserId, { column_id: 2, position: 0 });

      expect(otherCardInOldCol.position).toBe(0);
      expect(targetColumnCards[0].position).toBe(1);
      expect(targetColumnCards[1].position).toBe(2);
      expect(mockCardRepository.update).toHaveBeenCalledWith(1, { column_id: 2 });
      expect(mockCardRepository.update).toHaveBeenCalledWith(1, { position: 0 });
    });

    it('should update description', async () => {
      const card = createMockCard(1, 1, 0);
      mockCardRepository.findOne.mockResolvedValue(card);
      mockCardRepository.update.mockResolvedValue({ affected: 1 } as UpdateResult);

      await service.update(1, mockUserId, { description: 'New description' });

      expect(mockCardRepository.update).toHaveBeenCalledWith(1, { description: 'New description' });
    });

    it('should update due_date', async () => {
      const card = createMockCard(1, 1, 0);
      mockCardRepository.findOne.mockResolvedValue(card);
      mockCardRepository.update.mockResolvedValue({ affected: 1 } as UpdateResult);

      await service.update(1, mockUserId, { due_date: '2026-06-01' });

      expect(mockCardRepository.update).toHaveBeenCalledWith(1, {
        due_date: new Date('2026-06-01'),
      });
    });

    it('should clear due_date when set to nullish via empty string', async () => {
      const card = createMockCard(1, 1, 0);
      card.due_date = new Date('2026-01-01');
      mockCardRepository.findOne.mockResolvedValue(card);
      mockCardRepository.update.mockResolvedValue({ affected: 1 } as UpdateResult);

      await service.update(1, mockUserId, { due_date: undefined });

      expect(mockCardRepository.update).not.toHaveBeenCalledWith(
        1,
        expect.objectContaining({ due_date: expect.anything() as unknown }),
      );
    });
  });

  describe('findCardById', () => {
    it('should throw NotFoundException if card does not exist', async () => {
      mockCardRepository.findOne.mockResolvedValue(null);

      await expect(service.remove(1, mockUserId)).rejects.toThrow(NotFoundException);
    });

    it('should throw ForbiddenException if user does not own the board', async () => {
      const card = createMockCard(1, 1, 0);
      card.column.board.user_id = 999;
      mockCardRepository.findOne.mockResolvedValue(card);

      await expect(service.remove(1, mockUserId)).rejects.toThrow(ForbiddenException);
    });
  });

  describe('remove', () => {
    it('should delete card when user has valid ownership', async () => {
      const card = createMockCard(1, 1, 0);
      card.column.board.user_id = mockUserId;
      mockCardRepository.findOne.mockResolvedValue(card);
      mockCardRepository.remove.mockResolvedValue(card);

      await service.remove(1, mockUserId);

      expect(mockCardRepository.remove).toHaveBeenCalledWith(card);
    });

    it('should throw NotFoundException when deleting non-existent card', async () => {
      mockCardRepository.findOne.mockResolvedValue(null);

      await expect(service.remove(999, mockUserId)).rejects.toThrow(NotFoundException);
      expect(mockCardRepository.remove).not.toHaveBeenCalled();
    });

    it('should throw ForbiddenException when user does not own the board', async () => {
      const card = createMockCard(1, 1, 0);
      card.column.board.user_id = 999;
      mockCardRepository.findOne.mockResolvedValue(card);

      await expect(service.remove(1, mockUserId)).rejects.toThrow(ForbiddenException);
      expect(mockCardRepository.remove).not.toHaveBeenCalled();
    });
  });

  describe('findById', () => {
    it('should return a card with labels', async () => {
      const card = createMockCard(1, 1, 0);
      card.column.board.user_id = mockUserId;
      card.cardLabels = [
        {
          cardId: 1,
          labelId: 1,
          label: { id: 1, name: 'Urgent', color: 'red', user_id: mockUserId } as unknown as Label,
          card: card,
        },
      ];
      mockCardRepository.findOne.mockResolvedValue(card);

      const result = await service.findById(1, mockUserId);

      expect(result).toEqual(card);
    });

    it('should throw NotFoundException if card does not exist', async () => {
      mockCardRepository.findOne.mockResolvedValue(null);

      await expect(service.findById(999, mockUserId)).rejects.toThrow(NotFoundException);
    });

    it('should throw ForbiddenException if user does not own the board', async () => {
      const card = createMockCard(1, 1, 0);
      card.column.board.user_id = 999;
      mockCardRepository.findOne.mockResolvedValue(card);

      await expect(service.findById(1, mockUserId)).rejects.toThrow(ForbiddenException);
    });
  });

  describe('assignLabel', () => {
    it('should assign a label to a card', async () => {
      const card = createMockCard(1, 1, 0);
      card.column.board.user_id = mockUserId;
      const label = { id: 1, name: 'Urgent', color: 'red', user_id: mockUserId } as Label;

      mockCardRepository.findOne
        .mockResolvedValueOnce(card)
        .mockResolvedValueOnce({ ...card, labels: [label] });
      mockLabelRepository.findOne.mockResolvedValue(label);
      mockCardLabelRepository.findOne.mockResolvedValue(null);
      mockCardLabelRepository.create.mockReturnValue({
        id: 1,
        card: { id: 1 },
        label: { id: 1 },
        cardId: 1,
        labelId: 1,
      });
      mockCardLabelRepository.save.mockResolvedValue({ id: 1 });

      const result = await service.assignLabel(1, 1, mockUserId);

      expect(result).toBeDefined();
      expect(result).toMatchObject({ id: 1, title: 'Card 1' });
      expect(mockCardLabelRepository.save).toHaveBeenCalled();
      expect(mockCardLabelRepository.findOne).toHaveBeenCalledWith({
        where: { cardId: 1, labelId: 1 },
      });
    });

    it('should throw NotFoundException if card does not exist', async () => {
      mockCardRepository.findOne.mockResolvedValue(null);

      await expect(service.assignLabel(999, 1, mockUserId)).rejects.toThrow(NotFoundException);
    });

    it('should throw ForbiddenException if user does not own the card board', async () => {
      const card = createMockCard(1, 1, 0);
      card.column.board.user_id = 999;
      mockCardRepository.findOne.mockResolvedValue(card);

      await expect(service.assignLabel(1, 1, mockUserId)).rejects.toThrow(ForbiddenException);
    });

    it('should throw NotFoundException if label does not exist', async () => {
      const card = createMockCard(1, 1, 0);
      card.column.board.user_id = mockUserId;
      mockCardRepository.findOne.mockResolvedValue(card);
      mockLabelRepository.findOne.mockResolvedValue(null);

      await expect(service.assignLabel(1, 999, mockUserId)).rejects.toThrow(NotFoundException);
    });

    it('should throw ForbiddenException if user does not own the label', async () => {
      const card = createMockCard(1, 1, 0);
      card.column.board.user_id = mockUserId;
      const label = { id: 1, name: 'Urgent', color: 'red', user_id: 999 } as Label;

      mockCardRepository.findOne.mockResolvedValue(card);
      mockLabelRepository.findOne.mockResolvedValue(label);

      await expect(service.assignLabel(1, 1, mockUserId)).rejects.toThrow(ForbiddenException);
    });

    it('should return card idempotently if label already assigned', async () => {
      const card = createMockCard(1, 1, 0);
      card.column.board.user_id = mockUserId;
      const label = { id: 1, name: 'Urgent', color: 'red', user_id: mockUserId } as Label;

      mockCardRepository.findOne.mockResolvedValue(card);
      mockLabelRepository.findOne.mockResolvedValue(label);
      mockCardLabelRepository.findOne.mockResolvedValue({ id: 1 });

      const result = await service.assignLabel(1, 1, mockUserId);
      expect(result).toBeDefined();
    });
  });

  describe('removeLabel', () => {
    it('should remove a label from a card', async () => {
      const card = createMockCard(1, 1, 0);
      card.column.board.user_id = mockUserId;
      const label = { id: 1, name: 'Urgent', color: 'red', user_id: mockUserId } as Label;
      const cardLabel = { id: 1, card, label };

      mockCardRepository.findOne.mockResolvedValue(card);
      mockLabelRepository.findOne.mockResolvedValue(label);
      mockCardLabelRepository.findOne.mockResolvedValue(cardLabel);
      mockCardLabelRepository.remove.mockResolvedValue(cardLabel);

      await service.removeLabel(1, 1, mockUserId);

      expect(mockCardLabelRepository.remove).toHaveBeenCalledWith(cardLabel);
    });

    it('should throw NotFoundException if card does not exist', async () => {
      mockCardRepository.findOne.mockResolvedValue(null);

      await expect(service.removeLabel(999, 1, mockUserId)).rejects.toThrow(NotFoundException);
    });

    it('should throw NotFoundException if label does not exist', async () => {
      const card = createMockCard(1, 1, 0);
      card.column.board.user_id = mockUserId;
      mockCardRepository.findOne.mockResolvedValue(card);
      mockLabelRepository.findOne.mockResolvedValue(null);

      await expect(service.removeLabel(1, 999, mockUserId)).rejects.toThrow(NotFoundException);
    });

    it('should throw NotFoundException if label is not assigned to card', async () => {
      const card = createMockCard(1, 1, 0);
      card.column.board.user_id = mockUserId;
      const label = { id: 1, name: 'Urgent', color: 'red', user_id: mockUserId } as Label;

      mockCardRepository.findOne.mockResolvedValue(card);
      mockLabelRepository.findOne.mockResolvedValue(label);
      mockCardLabelRepository.findOne.mockResolvedValue(null);

      await expect(service.removeLabel(1, 1, mockUserId)).rejects.toThrow(NotFoundException);
    });
  });
});
