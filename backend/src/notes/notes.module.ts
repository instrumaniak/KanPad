import { Module } from '@nestjs/common';
import { TypeOrmModule } from '@nestjs/typeorm';
import { Note } from './entities/note.entity';
import { Tag } from '../tags/entities/tag.entity';
import { Board } from '../boards/entities/board.entity';
import { Project } from '../projects/entities/project.entity';
import { Card } from '../cards/entities/card.entity';
import { NotesService } from './notes.service';
import { NotesController } from './notes.controller';

@Module({
  imports: [TypeOrmModule.forFeature([Note, Tag, Board, Project, Card])],
  controllers: [NotesController],
  providers: [NotesService],
  exports: [NotesService],
})
export class NotesModule {}
