import {
  Controller,
  Get,
  Post,
  Patch,
  Delete,
  Body,
  Param,
  UseGuards,
  Session,
  ParseIntPipe,
  ValidationPipe,
  UsePipes,
} from '@nestjs/common';
import { ApiTags, ApiOperation, ApiResponse, ApiParam } from '@nestjs/swagger';
import { SessionGuard } from '../auth/guards/session.guard';
import { TagsService } from './tags.service';
import { CreateTagDto } from './dto/create-tag.dto';
import { UpdateTagDto } from './dto/update-tag.dto';
import { Tag } from './entities/tag.entity';

interface SessionData {
  userId: number;
}

interface TagResponse {
  id: number;
  name: string;
  color: string;
  user_id: number;
  created_at: string;
}

function toTagResponse(tag: Tag): TagResponse {
  return {
    id: tag.id,
    name: tag.name,
    color: tag.color,
    user_id: tag.user_id,
    created_at: tag.created_at.toISOString(),
  };
}

@Controller('api')
@UseGuards(SessionGuard)
@ApiTags('tags')
export class TagsController {
  constructor(private readonly tagsService: TagsService) {}

  @Get('tags')
  @ApiOperation({ summary: 'Get all tags for the current user' })
  @ApiResponse({ status: 200, description: 'Tags found' })
  async findAll(@Session() session: SessionData): Promise<{ data: TagResponse[] }> {
    const tags = await this.tagsService.findAll(session.userId);
    return { data: tags.map(toTagResponse) };
  }

  @Post('tags')
  @ApiOperation({ summary: 'Create a tag' })
  @ApiResponse({ status: 201, description: 'Tag created' })
  @UsePipes(new ValidationPipe({ transform: true }))
  async create(
    @Session() session: SessionData,
    @Body() dto: CreateTagDto,
  ): Promise<{ data: TagResponse; message: string }> {
    const tag = await this.tagsService.create(session.userId, dto);
    return { data: toTagResponse(tag), message: 'Tag created' };
  }

  @Patch('tags/:id')
  @ApiOperation({ summary: 'Update a tag' })
  @ApiParam({ name: 'id', type: Number })
  @ApiResponse({ status: 200, description: 'Tag updated' })
  @UsePipes(new ValidationPipe({ transform: true }))
  async update(
    @Session() session: SessionData,
    @Param('id', ParseIntPipe) id: number,
    @Body() dto: UpdateTagDto,
  ): Promise<{ data: TagResponse; message: string }> {
    const tag = await this.tagsService.update(id, session.userId, dto);
    return { data: toTagResponse(tag), message: 'Tag updated' };
  }

  @Delete('tags/:id')
  @ApiOperation({ summary: 'Delete a tag' })
  @ApiParam({ name: 'id', type: Number })
  @ApiResponse({ status: 200, description: 'Tag deleted' })
  async remove(
    @Session() session: SessionData,
    @Param('id', ParseIntPipe) id: number,
  ): Promise<{ message: string }> {
    await this.tagsService.remove(id, session.userId);
    return { message: 'Tag deleted' };
  }
}
