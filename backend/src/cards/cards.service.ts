import { Injectable, NotFoundException, ForbiddenException } from '@nestjs/common';
import { InjectRepository } from '@nestjs/typeorm';
import { DataSource, Repository } from 'typeorm';
import { Card } from './entities/card.entity';
import { CardLabel } from './entities/card-label.entity';
import { Label } from '../labels/entities/label.entity';
import { CreateCardDto } from './dto/create-card.dto';
import { UpdateCardDto } from './dto/update-card.dto';
import { BoardColumn } from '../columns/entities/column.entity';

@Injectable()
export class CardsService {
  constructor(
    @InjectRepository(Card)
    private readonly cardRepository: Repository<Card>,
    @InjectRepository(CardLabel)
    private readonly cardLabelRepository: Repository<CardLabel>,
    @InjectRepository(Label)
    private readonly labelRepository: Repository<Label>,
    @InjectRepository(BoardColumn)
    private readonly columnRepository: Repository<BoardColumn>,
    private readonly dataSource: DataSource,
  ) {}

  async create(userId: number, dto: CreateCardDto): Promise<Card> {
    await this.findColumnById(dto.column_id, userId);

    let position: number;
    if (dto.position !== undefined) {
      position = dto.position;
    } else {
      const maxPositionResult = await this.cardRepository
        .createQueryBuilder('card')
        .where('card.column_id = :columnId', { columnId: dto.column_id })
        .select('MAX(card.position)', 'max')
        .getRawOne<{ max: string | number | null }>();
      position = Number(maxPositionResult?.max ?? -1) + 1;
    }

    const card = this.cardRepository.create({
      title: dto.title,
      column_id: dto.column_id,
      position,
      description: dto.description ?? null,
      due_date: dto.due_date ? new Date(dto.due_date) : null,
    });

    return this.cardRepository.save(card);
  }

  async findAllByColumnId(columnId: number, userId: number): Promise<Card[]> {
    await this.findColumnById(columnId, userId);

    return this.cardRepository.find({
      where: { column_id: columnId },
      relations: ['cardLabels', 'cardLabels.label', 'checklists', 'checklists.items'],
      order: { position: 'ASC' },
    });
  }

  async update(id: number, userId: number, dto: UpdateCardDto): Promise<Card> {
    const card = await this.findCardById(id, userId);
    const oldColumnId = card.column_id;
    const oldPosition = card.position;

    await this.dataSource.transaction(async (manager) => {
      const cardRepo = manager.getRepository(Card);

      if (dto.title !== undefined) {
        await cardRepo.update(id, { title: dto.title });
      }

      if (dto.description !== undefined) {
        await cardRepo.update(id, { description: dto.description });
      }

      if (dto.due_date !== undefined) {
        await cardRepo.update(id, { due_date: dto.due_date ? new Date(dto.due_date) : null });
      }

      if (dto.position !== undefined) {
        const targetColumnId = dto.column_id ?? oldColumnId;
        const newPosition = dto.position;

        if (oldColumnId === targetColumnId && oldPosition !== newPosition) {
          await this.reorderWithinColumn(targetColumnId, oldPosition, newPosition, manager);
        } else if (oldColumnId !== targetColumnId) {
          await this.removeFromColumn(oldColumnId, oldPosition, manager);
          const maxPosition = await this.getMaxPositionInColumn(targetColumnId, manager);
          const targetPosition = Math.min(newPosition, maxPosition + 1);
          await this.insertIntoColumn(targetColumnId, targetPosition, manager);
        }
        await cardRepo.update(id, { position: dto.position });
      }

      if (dto.column_id !== undefined) {
        await this.findColumnById(dto.column_id, userId);
        await cardRepo.update(id, { column_id: dto.column_id });
      }
    });

    const updated = await this.cardRepository.findOne({ where: { id } });
    if (!updated) {
      throw new NotFoundException('Card not found after update');
    }
    return updated;
  }

  private async reorderWithinColumn(
    columnId: number,
    oldPos: number,
    newPos: number,
    manager?: import('typeorm').EntityManager,
  ): Promise<void> {
    if (oldPos === newPos) return;

    const cardRepo = manager ? manager.getRepository(Card) : this.cardRepository;
    const cards = await cardRepo.find({
      where: { column_id: columnId },
      order: { position: 'ASC' },
    });

    const maxPos = cards.length - 1;
    const safeOldPos = Math.max(0, Math.min(oldPos, maxPos));
    const safeNewPos = Math.max(0, Math.min(newPos, maxPos));

    if (safeOldPos === safeNewPos) return;

    const updates: Promise<Card>[] = [];
    for (const c of cards) {
      if (safeOldPos < safeNewPos && c.position > safeOldPos && c.position <= safeNewPos) {
        c.position -= 1;
        updates.push(cardRepo.save(c));
      } else if (safeOldPos > safeNewPos && c.position >= safeNewPos && c.position < safeOldPos) {
        c.position += 1;
        updates.push(cardRepo.save(c));
      }
    }
    await Promise.all(updates);
  }

  private async removeFromColumn(
    columnId: number,
    position: number,
    manager?: import('typeorm').EntityManager,
  ): Promise<void> {
    const cardRepo = manager ? manager.getRepository(Card) : this.cardRepository;
    const cards = await cardRepo.find({
      where: { column_id: columnId },
      order: { position: 'ASC' },
    });

    const updates: Promise<Card>[] = [];
    for (const c of cards) {
      if (c.position > position) {
        c.position -= 1;
        updates.push(cardRepo.save(c));
      }
    }
    await Promise.all(updates);
  }

  private async insertIntoColumn(
    columnId: number,
    position: number,
    manager?: import('typeorm').EntityManager,
  ): Promise<void> {
    const cardRepo = manager ? manager.getRepository(Card) : this.cardRepository;
    const cards = await cardRepo.find({
      where: { column_id: columnId },
      order: { position: 'ASC' },
    });

    const updates: Promise<Card>[] = [];
    for (const c of cards) {
      if (c.position >= position) {
        c.position += 1;
        updates.push(cardRepo.save(c));
      }
    }
    await Promise.all(updates);
  }

  private async getMaxPositionInColumn(
    columnId: number,
    manager?: import('typeorm').EntityManager,
  ): Promise<number> {
    const cardRepo = manager ? manager.getRepository(Card) : this.cardRepository;
    const result = await cardRepo
      .createQueryBuilder('card')
      .where('card.column_id = :columnId', { columnId })
      .select('MAX(card.position)', 'max')
      .getRawOne<{ max: string | number | null }>();
    return Number(result?.max ?? -1);
  }

  async findById(id: number, userId: number): Promise<Card> {
    const card = await this.cardRepository.findOne({
      where: { id },
      relations: [
        'column',
        'column.board',
        'cardLabels',
        'cardLabels.label',
        'checklists',
        'checklists.items',
      ],
    });

    if (!card) {
      throw new NotFoundException('Card not found');
    }

    if (card.column.board.user_id !== userId) {
      throw new ForbiddenException('Access denied');
    }

    return card;
  }

  async assignLabel(cardId: number, labelId: number, userId: number): Promise<Card> {
    return this.dataSource.transaction(async (manager) => {
      const card = await manager.getRepository(Card).findOne({
        where: { id: cardId },
        relations: ['column', 'column.board'],
      });

      if (!card) {
        throw new NotFoundException('Card not found');
      }

      if (card.column.board.user_id !== userId) {
        throw new ForbiddenException('Access denied');
      }

      const label = await manager.getRepository(Label).findOne({ where: { id: labelId } });

      if (!label) {
        throw new NotFoundException('Label not found');
      }

      if (label.user_id !== userId) {
        throw new ForbiddenException('Access denied');
      }

      const existing = await manager.getRepository(CardLabel).findOne({
        where: { cardId, labelId },
      });

      if (!existing) {
        const cardLabel = manager.getRepository(CardLabel).create({
          card: { id: cardId } as Card,
          label: { id: labelId } as Label,
          cardId,
          labelId,
        });
        await manager.getRepository(CardLabel).save(cardLabel);
      }

      return manager.getRepository(Card).findOne({
        where: { id: cardId },
        relations: ['cardLabels', 'cardLabels.label'],
      }) as Promise<Card>;
    });
  }

  async removeLabel(cardId: number, labelId: number, userId: number): Promise<void> {
    return this.dataSource.transaction(async (manager) => {
      const card = await manager.getRepository(Card).findOne({
        where: { id: cardId },
        relations: ['column', 'column.board'],
      });

      if (!card) {
        throw new NotFoundException('Card not found');
      }

      if (card.column.board.user_id !== userId) {
        throw new ForbiddenException('Access denied');
      }

      const label = await manager.getRepository(Label).findOne({ where: { id: labelId } });

      if (!label) {
        throw new NotFoundException('Label not found');
      }

      if (label.user_id !== userId) {
        throw new ForbiddenException('Access denied');
      }

      const existing = await manager.getRepository(CardLabel).findOne({
        where: { cardId, labelId },
      });

      if (!existing) {
        throw new NotFoundException('Label not assigned to this card');
      }

      await manager.getRepository(CardLabel).remove(existing);
    });
  }

  async remove(id: number, userId: number): Promise<void> {
    const card = await this.findCardById(id, userId);
    await this.cardRepository.remove(card);
  }

  private async findCardById(id: number, userId: number): Promise<Card> {
    const card = await this.cardRepository.findOne({
      where: { id },
      relations: ['column', 'column.board'],
    });

    if (!card) {
      throw new NotFoundException('Card not found');
    }

    if (card.column.board.user_id !== userId) {
      throw new ForbiddenException('Access denied');
    }

    return card;
  }

  private async findColumnById(id: number, userId: number): Promise<BoardColumn> {
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
}
