import {
  Entity,
  PrimaryGeneratedColumn,
  Column,
  CreateDateColumn,
  UpdateDateColumn,
  ManyToOne,
  ManyToMany,
  JoinTable,
  JoinColumn,
} from 'typeorm';
import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger';
import { Board } from '../../boards/entities/board.entity';
import { Project } from '../../projects/entities/project.entity';
import { Card } from '../../cards/entities/card.entity';
import { Tag } from '../../tags/entities/tag.entity';

@Entity('notes')
export class Note {
  @ApiProperty({ example: 1 })
  @PrimaryGeneratedColumn()
  id!: number;

  @ApiProperty({ example: 'Meeting notes' })
  @Column({ length: 255 })
  title!: string;

  @ApiProperty({ example: '# Markdown content' })
  @Column({ type: 'mediumtext', select: false })
  content!: string;

  @ApiPropertyOptional({ example: 1 })
  @Column({ type: 'int', nullable: true })
  board_id!: number | null;

  @ApiPropertyOptional({ example: 1 })
  @Column({ type: 'int', nullable: true })
  project_id!: number | null;

  @ApiPropertyOptional({ example: 1 })
  @Column({ type: 'int', nullable: true })
  card_id!: number | null;

  @ApiProperty({ example: 1 })
  @Column({ type: 'int' })
  user_id!: number;

  @ManyToOne(() => Board, { onDelete: 'CASCADE' })
  @JoinColumn({ name: 'board_id' })
  board!: Board | null;

  @ManyToOne(() => Project, { onDelete: 'CASCADE' })
  @JoinColumn({ name: 'project_id' })
  project!: Project | null;

  @ManyToOne(() => Card, { onDelete: 'CASCADE' })
  @JoinColumn({ name: 'card_id' })
  card!: Card | null;

  @ManyToMany(() => Tag, (tag) => tag.notes)
  @JoinTable({
    name: 'note_tags',
    joinColumn: { name: 'note_id', referencedColumnName: 'id' },
    inverseJoinColumn: { name: 'tag_id', referencedColumnName: 'id' },
  })
  tags!: Tag[];

  @ApiProperty({ example: '2026-01-01T00:00:00.000Z' })
  @CreateDateColumn()
  created_at!: Date;

  @ApiProperty({ example: '2026-01-01T00:00:00.000Z' })
  @UpdateDateColumn()
  updated_at!: Date;
}
