import {
  Controller,
  Get,
  Post,
  Patch,
  Delete,
  Body,
  Param,
  Query,
  UseGuards,
  Session,
  ParseIntPipe,
  ValidationPipe,
  UsePipes,
} from '@nestjs/common';
import { ApiTags, ApiOperation, ApiResponse, ApiParam } from '@nestjs/swagger';
import { SessionGuard } from '../auth/guards/session.guard';
import { NotesService } from './notes.service';
import { CreateNoteDto } from './dto/create-note.dto';
import { UpdateNoteDto } from './dto/update-note.dto';
import { ListNotesDto } from './dto/list-notes.dto';
import { Note } from './entities/note.entity';

interface SessionData {
  userId: number;
}

interface NoteResponse {
  id: number;
  title: string;
  content?: string;
  board_id: number | null;
  project_id: number | null;
  card_id: number | null;
  user_id: number;
  tags: { id: number; name: string; color: string }[];
  created_at: string;
  updated_at: string;
}

function toNoteResponse(note: Note): NoteResponse {
  return {
    id: note.id,
    title: note.title,
    content: note.content,
    board_id: note.board_id,
    project_id: note.project_id,
    card_id: note.card_id,
    user_id: note.user_id,
    tags:
      note.tags?.map((t) => ({
        id: t.id,
        name: t.name,
        color: t.color,
      })) || [],
    created_at: note.created_at.toISOString(),
    updated_at: note.updated_at.toISOString(),
  };
}

@Controller('api')
@UseGuards(SessionGuard)
@ApiTags('notes')
export class NotesController {
  constructor(private readonly notesService: NotesService) {}

  @Get('notes')
  @ApiOperation({ summary: 'Get all notes with optional filters' })
  @ApiResponse({ status: 200, description: 'Notes found' })
  async findAll(
    @Session() session: SessionData,
    @Query(new ValidationPipe({ transform: true })) query: ListNotesDto,
  ): Promise<{ data: NoteResponse[] }> {
    const notes = await this.notesService.findAll(session.userId, query);
    return { data: notes.map(toNoteResponse) };
  }

  @Post('notes')
  @ApiOperation({ summary: 'Create a note' })
  @ApiResponse({ status: 201, description: 'Note created' })
  @UsePipes(new ValidationPipe({ transform: true }))
  async create(
    @Session() session: SessionData,
    @Body() dto: CreateNoteDto,
  ): Promise<{ data: NoteResponse; message: string }> {
    const note = await this.notesService.create(session.userId, dto);
    return { data: toNoteResponse(note), message: 'Note created' };
  }

  @Get('notes/:id')
  @ApiOperation({ summary: 'Get a single note by id' })
  @ApiParam({ name: 'id', type: Number })
  @ApiResponse({ status: 200, description: 'Note found' })
  async findOne(
    @Session() session: SessionData,
    @Param('id', ParseIntPipe) id: number,
  ): Promise<{ data: NoteResponse }> {
    const note = await this.notesService.findById(id, session.userId);
    return { data: toNoteResponse(note) };
  }

  @Patch('notes/:id')
  @ApiOperation({ summary: 'Update a note' })
  @ApiParam({ name: 'id', type: Number })
  @ApiResponse({ status: 200, description: 'Note updated' })
  @UsePipes(new ValidationPipe({ transform: true }))
  async update(
    @Session() session: SessionData,
    @Param('id', ParseIntPipe) id: number,
    @Body() dto: UpdateNoteDto,
  ): Promise<{ data: NoteResponse; message: string }> {
    const note = await this.notesService.update(id, session.userId, dto);
    return { data: toNoteResponse(note), message: 'Note updated' };
  }

  @Delete('notes/:id')
  @ApiOperation({ summary: 'Delete a note' })
  @ApiParam({ name: 'id', type: Number })
  @ApiResponse({ status: 200, description: 'Note deleted' })
  async remove(
    @Session() session: SessionData,
    @Param('id', ParseIntPipe) id: number,
  ): Promise<{ message: string }> {
    await this.notesService.remove(id, session.userId);
    return { message: 'Note deleted' };
  }

  @Get('boards/:boardId/notes')
  @ApiOperation({ summary: 'Get notes linked to a board' })
  @ApiParam({ name: 'boardId', type: Number })
  @ApiResponse({ status: 200, description: 'Notes found' })
  async findByBoardId(
    @Session() session: SessionData,
    @Param('boardId', ParseIntPipe) boardId: number,
  ): Promise<{ data: NoteResponse[] }> {
    const notes = await this.notesService.findByBoardId(boardId, session.userId);
    return { data: notes.map(toNoteResponse) };
  }
}
