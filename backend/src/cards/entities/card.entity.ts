import {
  Entity,
  PrimaryGeneratedColumn,
  Column,
  CreateDateColumn,
  UpdateDateColumn,
  ManyToOne,
  JoinColumn,
  OneToMany,
} from 'typeorm';
import { ApiProperty } from '@nestjs/swagger';
import { BoardColumn } from '../../columns/entities/column.entity';
import { CardLabel } from './card-label.entity';
import { Checklist } from '../../checklists/entities/checklist.entity';

@Entity('cards')
export class Card {
  @ApiProperty({ example: 1 })
  @PrimaryGeneratedColumn()
  id!: number;

  @ApiProperty({ example: 'Buy groceries' })
  @Column({ length: 255 })
  title!: string;

  @ApiProperty({ example: 1 })
  @Column()
  column_id!: number;

  @ManyToOne(() => BoardColumn, (column) => column.cards, { onDelete: 'CASCADE' })
  @JoinColumn({ name: 'column_id' })
  column!: BoardColumn;

  @ApiProperty({ example: 0 })
  @Column({ default: 0 })
  position!: number;

  @ApiProperty({ example: 'Card description', required: false })
  @Column({ type: 'text', nullable: true })
  description!: string | null;

  @ApiProperty({ example: '2026-01-01T00:00:00.000Z', required: false })
  @Column({ type: 'datetime', nullable: true })
  due_date!: Date | null;

  @ApiProperty({ example: '2026-01-01T00:00:00.000Z' })
  @CreateDateColumn()
  created_at!: Date;

  @ApiProperty({ example: '2026-01-01T00:00:00.000Z' })
  @UpdateDateColumn()
  updated_at!: Date;

  @OneToMany(() => CardLabel, (cardLabel) => cardLabel.card)
  cardLabels!: CardLabel[];

  @OneToMany(() => Checklist, (checklist) => checklist.card)
  checklists!: Checklist[];

  checklist_progress?: { completed: number; total: number; percent: number };

  // Transient, populated by list queries via SQL CASE — never selected as TEXT.
  has_description?: boolean;
}
