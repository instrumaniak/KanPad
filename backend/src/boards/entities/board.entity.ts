import {
  Entity,
  PrimaryGeneratedColumn,
  Column,
  CreateDateColumn,
  UpdateDateColumn,
  ManyToOne,
  OneToMany,
  JoinColumn,
} from 'typeorm';
import { ApiProperty } from '@nestjs/swagger';
import { User } from '../../users/entities/user.entity';
import { Project } from '../../projects/entities/project.entity';
import { BoardColumn } from '../../columns/entities/column.entity';
import { Note } from '../../notes/entities/note.entity';

@Entity('boards')
export class Board {
  @ApiProperty({ example: 1 })
  @PrimaryGeneratedColumn()
  id!: number;

  @ApiProperty({ example: 'My Board' })
  @Column({ length: 255 })
  name!: string;

  @ApiProperty({ example: '#0079BF' })
  @Column({ length: 7, default: '#0079BF' })
  background_color!: string;

  @ApiProperty({ example: 1 })
  @Column()
  user_id!: number;

  @ApiProperty({ example: 1, nullable: true })
  @Column({ nullable: true })
  project_id!: number | null;

  @ApiProperty({ example: false })
  @Column({ default: false })
  is_archived!: boolean;

  @ManyToOne(() => User, (user) => user.boards, { onDelete: 'CASCADE' })
  @JoinColumn({ name: 'user_id' })
  user!: User;

  @ManyToOne(() => Project, (project) => project.boards, { onDelete: 'SET NULL' })
  @JoinColumn({ name: 'project_id' })
  project!: Project | null;

  @ApiProperty({ example: '2026-01-01T00:00:00.000Z' })
  @CreateDateColumn()
  created_at!: Date;

  @ApiProperty({ example: '2026-01-01T00:00:00.000Z' })
  @UpdateDateColumn()
  updated_at!: Date;

  @OneToMany(() => BoardColumn, (column) => column.board)
  columns!: BoardColumn[];

  @OneToMany(() => Note, (note) => note.board, { cascade: true })
  notes!: Note[];
}
