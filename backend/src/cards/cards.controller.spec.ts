import { Test, TestingModule } from '@nestjs/testing';
import { CardsController } from './cards.controller';
import { CardsService } from './cards.service';
import { Card } from './entities/card.entity';

describe('CardsController', () => {
  let controller: CardsController;

  type CardPayload = {
    id: number;
    title: string;
    column_id: number;
    position: number;
    description: string | null;
    due_date: string | null;
    labels: { id: number; name: string; color: string }[];
    created_at: string;
    updated_at: string;
  };

  type CardSummaryPayload = Omit<CardPayload, 'description'> & {
    has_description: boolean;
  };

  type CardListResponse = { data: CardSummaryPayload[] };
  type CardMutationResponse = { data: CardPayload; message: string };

  const mockCardsService: jest.Mocked<
    Pick<
      CardsService,
      | 'create'
      | 'findAllByColumnId'
      | 'update'
      | 'remove'
      | 'findById'
      | 'assignLabel'
      | 'removeLabel'
    >
  > = {
    create: jest.fn(),
    findAllByColumnId: jest.fn(),
    update: jest.fn(),
    remove: jest.fn(),
    findById: jest.fn(),
    assignLabel: jest.fn(),
    removeLabel: jest.fn(),
  };

  beforeEach(async () => {
    const module: TestingModule = await Test.createTestingModule({
      controllers: [CardsController],
      providers: [{ provide: CardsService, useValue: mockCardsService }],
    }).compile();

    controller = module.get<CardsController>(CardsController);
  });

  afterEach(() => {
    jest.clearAllMocks();
  });

  describe('findAll', () => {
    it('should return card summaries with has_description and no description', async () => {
      const cards = [
        {
          id: 1,
          title: 'Card 1',
          column_id: 1,
          position: 0,
          has_description: false,
          due_date: null,
          created_at: new Date(),
          updated_at: new Date(),
          cardLabels: [],
        },
        {
          id: 2,
          title: 'Card 2',
          column_id: 1,
          position: 1,
          description: 'full TEXT that must never leak on lists',
          has_description: true,
          due_date: new Date('2026-01-01'),
          created_at: new Date(),
          updated_at: new Date(),
          cardLabels: [],
        },
      ];
      mockCardsService.findAllByColumnId.mockResolvedValue(cards as unknown as Card[]);

      const result = (await controller.findAll({ userId: 1 }, 1)) as CardListResponse;

      expect(result.data).toHaveLength(2);
      expect(result.data).toEqual(
        expect.arrayContaining([
          expect.objectContaining({ id: 1, title: 'Card 1', has_description: false, due_date: null }),
          expect.objectContaining({
            id: 2,
            title: 'Card 2',
            has_description: true,
            due_date: expect.any(String) as unknown,
          }),
        ]),
      );
      // Summary contract: no description TEXT leaked on lists
      for (const card of result.data) {
        expect(card).not.toHaveProperty('description');
      }
    });
  });

  describe('create', () => {
    it('should create a card and return response', async () => {
      const card = {
        id: 1,
        title: 'New Card',
        column_id: 1,
        position: 0,
        description: null,
        due_date: null,
        created_at: new Date(),
        updated_at: new Date(),
      };
      mockCardsService.create.mockResolvedValue(card as Card);

      const result = (await controller.create(
        { userId: 1 },
        { title: 'New Card', column_id: 1 },
      )) as CardMutationResponse;

      expect(result.data).toMatchObject({
        id: 1,
        title: 'New Card',
        column_id: 1,
        position: 0,
        description: null,
        due_date: null,
      });
      expect(result.message).toBe('Card created');
      expect(result.data.created_at).toEqual(expect.any(String) as unknown);
      expect(result.data.updated_at).toEqual(expect.any(String) as unknown);
    });

    it('should create a card with description and due_date', async () => {
      const card = {
        id: 1,
        title: 'New Card',
        column_id: 1,
        position: 0,
        description: 'A description',
        due_date: new Date('2026-01-01'),
        created_at: new Date(),
        updated_at: new Date(),
      };
      mockCardsService.create.mockResolvedValue(card as Card);

      const result = (await controller.create(
        { userId: 1 },
        { title: 'New Card', column_id: 1, description: 'A description', due_date: '2026-01-01' },
      )) as CardMutationResponse;

      expect(result.data).toMatchObject({
        id: 1,
        title: 'New Card',
        description: 'A description',
        due_date: expect.any(String) as unknown,
      });
    });
  });

  describe('update', () => {
    it('should update a card and return response', async () => {
      const card = {
        id: 1,
        title: 'Updated Card',
        column_id: 1,
        position: 0,
        description: null,
        due_date: null,
        created_at: new Date(),
        updated_at: new Date(),
      };
      mockCardsService.update.mockResolvedValue(card as Card);

      const result = (await controller.update({ userId: 1 }, 1, {
        title: 'Updated Card',
      })) as CardMutationResponse;

      expect(result.data).toMatchObject({
        id: 1,
        title: 'Updated Card',
        column_id: 1,
        position: 0,
        description: null,
        due_date: null,
      });
      expect(result.message).toBe('Card updated');
      expect(result.data.created_at).toEqual(expect.any(String) as unknown);
      expect(result.data.updated_at).toEqual(expect.any(String) as unknown);
    });

    it('should update description and due_date', async () => {
      const card = {
        id: 1,
        title: 'Card',
        column_id: 1,
        position: 0,
        description: 'Updated description',
        due_date: new Date('2026-06-01'),
        created_at: new Date(),
        updated_at: new Date(),
      };
      mockCardsService.update.mockResolvedValue(card as Card);

      const result = (await controller.update({ userId: 1 }, 1, {
        description: 'Updated description',
        due_date: '2026-06-01',
      })) as CardMutationResponse;

      expect(result.data).toMatchObject({
        description: 'Updated description',
        due_date: expect.any(String) as unknown,
      });
    });
  });

  describe('remove', () => {
    it('should delete a card and return message', async () => {
      mockCardsService.remove.mockResolvedValue();

      const result = await controller.remove({ userId: 1 }, 1);

      expect(result).toEqual({ message: 'Card deleted' });
    });
  });

  describe('findOne', () => {
    it('should return a card by id with labels', async () => {
      const card = {
        id: 1,
        title: 'Card 1',
        column_id: 1,
        position: 0,
        description: null,
        due_date: null,
        cardLabels: [{ label: { id: 1, name: 'Urgent', color: 'red' } }],
        created_at: new Date(),
        updated_at: new Date(),
      };
      mockCardsService.findById.mockResolvedValue(card as Card);

      const result = await controller.findOne({ userId: 1 }, 1);

      expect(result.data).toMatchObject({
        id: 1,
        title: 'Card 1',
        labels: [{ id: 1, name: 'Urgent', color: 'red' }],
      });
    });
  });

  describe('assignLabel', () => {
    it('should assign a label to a card and return card', async () => {
      const card = {
        id: 1,
        title: 'Card 1',
        column_id: 1,
        position: 0,
        description: null,
        due_date: null,
        cardLabels: [{ label: { id: 1, name: 'Urgent', color: 'red' } }],
        created_at: new Date(),
        updated_at: new Date(),
      };
      mockCardsService.assignLabel.mockResolvedValue(card as Card);

      const result = (await controller.assignLabel({ userId: 1 }, 1, 1)) as CardMutationResponse;

      expect(result.data).toMatchObject({ id: 1, title: 'Card 1' });
      expect(result.message).toBe('Label assigned');
      expect(mockCardsService.assignLabel).toHaveBeenCalledWith(1, 1, 1);
    });
  });

  describe('removeLabel', () => {
    it('should remove a label from a card and return message', async () => {
      mockCardsService.removeLabel.mockResolvedValue(undefined);

      const result = await controller.removeLabel({ userId: 1 }, 1, 1);

      expect(result).toEqual({ message: 'Label removed' });
      expect(mockCardsService.removeLabel).toHaveBeenCalledWith(1, 1, 1);
    });
  });
});
