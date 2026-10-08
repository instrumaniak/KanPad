import { Repository } from 'typeorm';
import {
  attachCardListEnrichments,
  attachChecklistProgress,
  attachDescriptionFlags,
  buildCardSummaryQuery,
  CARD_SUMMARY_SELECT,
} from './card-summary.helpers';
import { Card } from '../cards/entities/card.entity';

type MockQb = {
  select: jest.Mock;
  addSelect: jest.Mock;
  where: jest.Mock;
  getRawMany: jest.Mock;
  leftJoinAndSelect: jest.Mock;
};

function createQb(): MockQb {
  const qb = {} as MockQb;
  qb.select = jest.fn().mockReturnValue(qb);
  qb.addSelect = jest.fn().mockReturnValue(qb);
  qb.where = jest.fn().mockReturnValue(qb);
  qb.getRawMany = jest.fn();
  qb.leftJoinAndSelect = jest.fn().mockReturnValue(qb);
  return qb;
}

function createRepo(
  qb?: MockQb,
  query?: jest.Mock,
): { repo: Repository<Card>; createQueryBuilder: jest.Mock; query: jest.Mock } {
  const createQueryBuilder = jest.fn().mockReturnValue(qb);
  const queryMock = query ?? jest.fn();
  const repo = { createQueryBuilder, query: queryMock } as unknown as Repository<Card>;
  return { repo, createQueryBuilder, query: queryMock };
}

describe('card-summary.helpers', () => {
  const createCard = (id: number): Card => ({ id }) as Card;

  describe('CARD_SUMMARY_SELECT', () => {
    it('never selects description TEXT', () => {
      expect(CARD_SUMMARY_SELECT.join(' ')).not.toContain('description');
      expect(CARD_SUMMARY_SELECT).toEqual(
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
    });
  });

  describe('buildCardSummaryQuery', () => {
    it('applies select list and label joins', () => {
      const qb = createQb();
      const { repo, createQueryBuilder } = createRepo(qb);

      const result = buildCardSummaryQuery(repo);

      expect(createQueryBuilder).toHaveBeenCalledWith('card');
      expect(qb.select).toHaveBeenCalledWith(CARD_SUMMARY_SELECT);
      expect(qb.leftJoinAndSelect).toHaveBeenCalledWith('card.cardLabels', 'cardLabels');
      expect(qb.leftJoinAndSelect).toHaveBeenCalledWith('cardLabels.label', 'label');
      expect(result).toBe(qb);
    });
  });

  describe('attachDescriptionFlags', () => {
    it('maps numeric flags to boolean', async () => {
      const cards = [createCard(1), createCard(2)];
      const qb = createQb();
      qb.getRawMany.mockResolvedValue([
        { id: 1, has_description: 0 },
        { id: 2, has_description: 1 },
      ]);
      const { repo } = createRepo(qb);

      await attachDescriptionFlags(repo, cards);

      expect(cards[0].has_description).toBe(false);
      expect(cards[1].has_description).toBe(true);
      expect(qb.addSelect).toHaveBeenCalledWith(
        expect.stringContaining('CASE WHEN'),
        'has_description',
      );
    });

    it('coerces string flags from MySQL raw results', async () => {
      const cards = [createCard(1), createCard(2)];
      const qb = createQb();
      qb.getRawMany.mockResolvedValue([
        { id: '1', has_description: '0' },
        { id: '2', has_description: '1' },
      ]);
      const { repo } = createRepo(qb);

      await attachDescriptionFlags(repo, cards);

      expect(cards[0].has_description).toBe(false);
      expect(cards[1].has_description).toBe(true);
    });

    it('defaults missing flags to false and no-ops on empty', async () => {
      const cards = [createCard(9)];
      const qb = createQb();
      qb.getRawMany.mockResolvedValue([]);
      const { repo } = createRepo(qb);

      await attachDescriptionFlags(repo, cards);
      expect(cards[0].has_description).toBe(false);

      const { repo: emptyRepo, createQueryBuilder } = createRepo();
      await attachDescriptionFlags(emptyRepo, []);
      expect(createQueryBuilder).not.toHaveBeenCalled();
    });
  });

  describe('attachChecklistProgress', () => {
    it('computes percent and leaves missing cards undefined', async () => {
      const cards = [createCard(1), createCard(2)];
      const query = jest.fn().mockResolvedValue([{ card_id: 1, total: 3, completed: 1 }]);
      const repo = { query } as unknown as Repository<Card>;

      await attachChecklistProgress(repo, cards);

      expect(cards[0].checklist_progress).toEqual({ completed: 1, total: 3, percent: 33 });
      expect(cards[1].checklist_progress).toBeUndefined();
    });

    it('handles zero totals as 0 percent', async () => {
      const cards = [createCard(1)];
      const query = jest.fn().mockResolvedValue([{ card_id: 1, total: 0, completed: 0 }]);
      const repo = { query } as unknown as Repository<Card>;

      await attachChecklistProgress(repo, cards);

      expect(cards[0].checklist_progress).toEqual({ completed: 0, total: 0, percent: 0 });
    });

    it('no-ops on empty without querying', async () => {
      const query = jest.fn();
      const repo = { query } as unknown as Repository<Card>;
      await attachChecklistProgress(repo, []);
      expect(query).not.toHaveBeenCalled();
    });
  });

  describe('attachCardListEnrichments', () => {
    it('no-ops on empty without querying', async () => {
      const createQueryBuilder = jest.fn();
      const query = jest.fn();
      const repo = { createQueryBuilder, query } as unknown as Repository<Card>;
      await attachCardListEnrichments(repo, []);
      expect(createQueryBuilder).not.toHaveBeenCalled();
      expect(query).not.toHaveBeenCalled();
    });

    it('attaches both flags and progress', async () => {
      const cards = [createCard(1), createCard(2)];
      const qb = createQb();
      qb.getRawMany.mockResolvedValue([
        { id: 1, has_description: 1 },
        { id: 2, has_description: 0 },
      ]);
      const query = jest.fn().mockResolvedValue([{ card_id: 1, total: 2, completed: 1 }]);
      const repo = {
        createQueryBuilder: jest.fn().mockReturnValue(qb),
        query,
      } as unknown as Repository<Card>;

      await attachCardListEnrichments(repo, cards);

      expect(cards[0].has_description).toBe(true);
      expect(cards[1].has_description).toBe(false);
      expect(cards[0].checklist_progress).toEqual({ completed: 1, total: 2, percent: 50 });
    });
  });
});
