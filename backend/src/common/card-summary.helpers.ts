import { Repository, SelectQueryBuilder } from 'typeorm';
import { Card } from '../cards/entities/card.entity';

// Columns selected on card list queries. Never include `description` TEXT here;
// `has_description` is computed separately via SQL CASE (boolean only).
export const CARD_SUMMARY_SELECT = [
  'card.id',
  'card.title',
  'card.column_id',
  'card.position',
  'card.due_date',
  'card.created_at',
  'card.updated_at',
];

// Shared base query for card summaries (labels joined, no description TEXT).
export function buildCardSummaryQuery(cardRepository: Repository<Card>): SelectQueryBuilder<Card> {
  return cardRepository
    .createQueryBuilder('card')
    .select(CARD_SUMMARY_SELECT)
    .leftJoinAndSelect('card.cardLabels', 'cardLabels')
    .leftJoinAndSelect('cardLabels.label', 'label');
}

export async function attachDescriptionFlags(
  cardRepository: Repository<Card>,
  cards: Card[],
): Promise<void> {
  if (cards.length === 0) return;

  const ids = cards.map((c) => c.id);
  const flags: { id: number; has_description: number | string }[] = await cardRepository
    .createQueryBuilder('card')
    .select('card.id', 'id')
    .addSelect(
      `CASE WHEN card.description IS NOT NULL AND card.description REGEXP '[^[:space:]]' THEN 1 ELSE 0 END`,
      'has_description',
    )
    .where('card.id IN (:...ids)', { ids })
    .getRawMany();

  const flagMap = new Map<number, boolean>();
  for (const row of flags) {
    flagMap.set(Number(row.id), Number(row.has_description) === 1);
  }
  for (const card of cards) {
    card.has_description = flagMap.get(card.id) ?? false;
  }
}

export async function attachChecklistProgress(
  cardRepository: Repository<Card>,
  cards: Card[],
): Promise<void> {
  const ids = cards.map((c) => c.id);
  if (ids.length === 0) return;

  const placeholders = ids.map(() => '?').join(',');
  const progressRaw: { card_id: number; total: number; completed: number }[] =
    await cardRepository.query(
      `SELECT cl.card_id, COUNT(ci.id) AS total, COALESCE(SUM(ci.is_completed), 0) AS completed
         FROM checklists cl
         LEFT JOIN checklist_items ci ON ci.checklist_id = cl.id
         WHERE cl.card_id IN (${placeholders})
         GROUP BY cl.card_id`,
      ids,
    );

  const progressMap = new Map<number, { completed: number; total: number; percent: number }>();
  for (const row of progressRaw) {
    const total = Number(row.total);
    const completed = Number(row.completed);
    progressMap.set(Number(row.card_id), {
      completed,
      total,
      percent: total === 0 ? 0 : Math.round((completed / total) * 100),
    });
  }

  for (const card of cards) {
    card.checklist_progress = progressMap.get(card.id);
  }
}

// Convenience: runs both enrichment steps, no-op on empty lists.
export async function attachCardListEnrichments(
  cardRepository: Repository<Card>,
  cards: Card[],
): Promise<void> {
  if (cards.length === 0) return;
  await attachDescriptionFlags(cardRepository, cards);
  await attachChecklistProgress(cardRepository, cards);
}
