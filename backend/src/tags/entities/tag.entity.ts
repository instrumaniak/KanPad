import { Entity, PrimaryGeneratedColumn, Column, CreateDateColumn, ManyToMany } from 'typeorm';
import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger';
import { Note } from '../../notes/entities/note.entity';

@Entity('tags')
export class Tag {
  @ApiProperty({ example: 1 })
  @PrimaryGeneratedColumn()
  id!: number;

  @ApiProperty({ example: 'important' })
  @Column({ length: 50 })
  name!: string;

  @ApiPropertyOptional({ example: 'teal' })
  @Column({ length: 20, default: 'teal' })
  color!: string;

  @ApiProperty({ example: 1 })
  @Column()
  user_id!: number;

  @ManyToMany(() => Note, (note) => note.tags)
  notes!: Note[];

  @ApiProperty({ example: '2026-01-01T00:00:00.000Z' })
  @CreateDateColumn()
  created_at!: Date;
}
