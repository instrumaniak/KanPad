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
import { ApiTags, ApiOperation, ApiResponse, ApiParam, ApiQuery } from '@nestjs/swagger';
import { SessionGuard } from '../auth/guards/session.guard';
import { BoardsService } from './boards.service';
import { CreateBoardDto } from './dto/create-board.dto';
import { UpdateBoardDto } from './dto/update-board.dto';

interface SessionData {
  userId: number;
}

interface BoardResponse {
  id: number;
  name: string;
  background_color: string;
  project_id: number | null;
  project?: { id: number; name: string } | null;
  created_at: string;
  updated_at: string;
  columns?: { id: number; name: string; position: number }[];
  is_archived?: boolean;
  view_mode: 'board' | 'list';
}

@Controller('api/boards')
@UseGuards(SessionGuard)
@ApiTags('boards')
export class BoardsController {
  constructor(private readonly boardsService: BoardsService) {}

  @Get()
  @ApiOperation({ summary: 'Get all boards for the authenticated user' })
  @ApiResponse({ status: 200, description: 'List of boards' })
  @ApiQuery({ name: 'projectId', required: false, type: Number })
  @ApiQuery({ name: 'archived', required: false, type: Boolean })
  async findAll(
    @Session() session: SessionData,
    @Query('projectId') projectId?: string,
    @Query('archived') archived?: string,
  ): Promise<{ data: BoardResponse[]; total: number }> {
    const projectIdNum = projectId ? parseInt(projectId, 10) : undefined;
    const includeArchived = archived === 'true';
    const boards = await this.boardsService.findAllByUserId(
      session.userId,
      projectIdNum,
      includeArchived,
    );
    const data: BoardResponse[] = boards.map((b) => ({
      id: b.id,
      name: b.name,
      background_color: b.background_color,
      project_id: b.project_id,
      project: b.project ? { id: b.project.id, name: b.project.name } : null,
      created_at: b.created_at.toISOString(),
      updated_at: b.updated_at.toISOString(),
      is_archived: b.is_archived,
      view_mode: b.view_mode ?? 'board',
    }));
    return { data, total: data.length };
  }

  @Get('archived')
  @ApiOperation({ summary: 'Get all archived boards for the authenticated user' })
  @ApiResponse({ status: 200, description: 'List of archived boards' })
  async findArchived(
    @Session() session: SessionData,
  ): Promise<{ data: BoardResponse[]; total: number }> {
    const boards = await this.boardsService.findAllArchivedByUserId(session.userId);
    const data: BoardResponse[] = boards.map((b) => ({
      id: b.id,
      name: b.name,
      background_color: b.background_color,
      project_id: b.project_id,
      project: b.project ? { id: b.project.id, name: b.project.name } : null,
      created_at: b.created_at.toISOString(),
      updated_at: b.updated_at.toISOString(),
      is_archived: b.is_archived,
      view_mode: b.view_mode ?? 'board',
    }));
    return { data, total: data.length };
  }

  @Post()
  @ApiOperation({ summary: 'Create a new board with default columns' })
  @ApiResponse({ status: 201, description: 'Board created' })
  @UsePipes(new ValidationPipe({ transform: true }))
  async create(
    @Session() session: SessionData,
    @Body() dto: CreateBoardDto,
  ): Promise<{ data: BoardResponse; message: string }> {
    const board = await this.boardsService.create(session.userId, dto);
    const data: BoardResponse = {
      id: board.id,
      name: board.name,
      background_color: board.background_color,
      project_id: board.project_id,
      project: board.project ? { id: board.project.id, name: board.project.name } : null,
      created_at: board.created_at.toISOString(),
      updated_at: board.updated_at.toISOString(),
      columns: board.columns.map((c) => ({
        id: c.id,
        name: c.name,
        position: c.position,
      })),
      is_archived: board.is_archived,
      view_mode: board.view_mode ?? 'board',
    };
    return { data, message: 'Board created successfully' };
  }

  @Get(':id')
  @ApiOperation({ summary: 'Get a single board' })
  @ApiParam({ name: 'id', type: Number })
  @ApiResponse({ status: 200, description: 'Board details' })
  async findOne(
    @Session() session: SessionData,
    @Param('id', ParseIntPipe) id: number,
  ): Promise<{ data: BoardResponse }> {
    const board = await this.boardsService.findOne(id, session.userId);
    const data: BoardResponse = {
      id: board.id,
      name: board.name,
      background_color: board.background_color,
      project_id: board.project_id,
      project: board.project ? { id: board.project.id, name: board.project.name } : null,
      created_at: board.created_at.toISOString(),
      updated_at: board.updated_at.toISOString(),
      columns: board.columns.map((c) => ({
        id: c.id,
        name: c.name,
        position: c.position,
      })),
      is_archived: board.is_archived,
      view_mode: board.view_mode ?? 'board',
    };
    return { data };
  }

  @Patch(':id')
  @ApiOperation({ summary: 'Update board name, color, project, or view mode' })
  @ApiParam({ name: 'id', type: Number })
  @ApiResponse({ status: 200, description: 'Board updated' })
  @UsePipes(new ValidationPipe({ transform: true }))
  async update(
    @Session() session: SessionData,
    @Param('id', ParseIntPipe) id: number,
    @Body() dto: UpdateBoardDto,
  ): Promise<{ data: BoardResponse; message: string }> {
    const board = await this.boardsService.update(id, session.userId, dto);
    const data: BoardResponse = {
      id: board.id,
      name: board.name,
      background_color: board.background_color,
      project_id: board.project_id,
      project: board.project ? { id: board.project.id, name: board.project.name } : null,
      created_at: board.created_at.toISOString(),
      updated_at: board.updated_at.toISOString(),
      is_archived: board.is_archived,
      view_mode: board.view_mode ?? 'board',
    };
    return { data, message: 'Board updated' };
  }

  @Delete(':id')
  @ApiOperation({ summary: 'Delete a board and all its columns/cards' })
  @ApiParam({ name: 'id', type: Number })
  @ApiResponse({ status: 200, description: 'Board deleted' })
  async remove(
    @Session() session: SessionData,
    @Param('id', ParseIntPipe) id: number,
  ): Promise<{ message: string }> {
    await this.boardsService.remove(id, session.userId);
    return { message: 'Board deleted' };
  }

  @Patch(':id/archive')
  @ApiOperation({ summary: 'Archive a board' })
  @ApiParam({ name: 'id', type: Number })
  @ApiResponse({ status: 200, description: 'Board archived' })
  async archive(
    @Session() session: SessionData,
    @Param('id', ParseIntPipe) id: number,
  ): Promise<{ data: BoardResponse; message: string }> {
    const board = await this.boardsService.archive(id, session.userId);
    const data: BoardResponse = {
      id: board.id,
      name: board.name,
      background_color: board.background_color,
      project_id: board.project_id,
      project: board.project ? { id: board.project.id, name: board.project.name } : null,
      created_at: board.created_at.toISOString(),
      updated_at: board.updated_at.toISOString(),
      is_archived: board.is_archived,
      view_mode: board.view_mode ?? 'board',
    };
    return { data, message: 'Board archived' };
  }

  @Patch(':id/restore')
  @ApiOperation({ summary: 'Restore an archived board' })
  @ApiParam({ name: 'id', type: Number })
  @ApiResponse({ status: 200, description: 'Board restored' })
  async restore(
    @Session() session: SessionData,
    @Param('id', ParseIntPipe) id: number,
  ): Promise<{ data: BoardResponse; message: string }> {
    const board = await this.boardsService.restore(id, session.userId);
    const data: BoardResponse = {
      id: board.id,
      name: board.name,
      background_color: board.background_color,
      project_id: board.project_id,
      project: board.project ? { id: board.project.id, name: board.project.name } : null,
      created_at: board.created_at.toISOString(),
      updated_at: board.updated_at.toISOString(),
      is_archived: board.is_archived,
      view_mode: board.view_mode ?? 'board',
    };
    return { data, message: 'Board restored' };
  }

  @Delete(':id/permanent')
  @ApiOperation({ summary: 'Permanently delete an archived board' })
  @ApiParam({ name: 'id', type: Number })
  @ApiResponse({ status: 200, description: 'Board permanently deleted' })
  async permanentDelete(
    @Session() session: SessionData,
    @Param('id', ParseIntPipe) id: number,
  ): Promise<{ message: string }> {
    await this.boardsService.permanentDelete(id, session.userId);
    return { message: 'Board permanently deleted' };
  }
}
