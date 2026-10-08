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
  HttpCode,
} from '@nestjs/common';
import { ApiTags, ApiOperation, ApiResponse, ApiParam } from '@nestjs/swagger';
import { SessionGuard } from '../auth/guards/session.guard';
import { CardsService } from './cards.service';
import { CreateCardDto } from './dto/create-card.dto';
import { UpdateCardDto } from './dto/update-card.dto';
import {
  CardResponse,
  CardDetailResponse,
  CardSummaryResponse,
  toCardResponse,
  toCardDetailResponse,
  toCardSummaryResponse,
} from './dto/card-response.dto';

interface SessionData {
  userId: number;
}

@Controller('api')
@UseGuards(SessionGuard)
@ApiTags('cards')
export class CardsController {
  constructor(private readonly cardsService: CardsService) {}

  @Get('columns/:columnId/cards')
  @ApiOperation({ summary: 'Get all cards for a column' })
  @ApiParam({ name: 'columnId', type: Number })
  @ApiResponse({ status: 200, description: 'List of cards' })
  async findAll(
    @Session() session: SessionData,
    @Param('columnId', ParseIntPipe) columnId: number,
  ): Promise<{ data: CardSummaryResponse[] }> {
    const cards = await this.cardsService.findAllByColumnId(columnId, session.userId);
    return { data: cards.map(toCardSummaryResponse) };
  }

  @Post('cards')
  @ApiOperation({ summary: 'Create a new card' })
  @ApiResponse({ status: 201, description: 'Card created' })
  @UsePipes(new ValidationPipe({ transform: true }))
  async create(
    @Session() session: SessionData,
    @Body() dto: CreateCardDto,
  ): Promise<{ data: CardResponse; message: string }> {
    const card = await this.cardsService.create(session.userId, dto);
    return { data: toCardResponse(card), message: 'Card created' };
  }

  @Patch('cards/:id')
  @ApiOperation({ summary: 'Update a card' })
  @ApiParam({ name: 'id', type: Number })
  @ApiResponse({ status: 200, description: 'Card updated' })
  @UsePipes(new ValidationPipe({ transform: true }))
  async update(
    @Session() session: SessionData,
    @Param('id', ParseIntPipe) id: number,
    @Body() dto: UpdateCardDto,
  ): Promise<{ data: CardResponse; message: string }> {
    const card = await this.cardsService.update(id, session.userId, dto);
    return { data: toCardResponse(card), message: 'Card updated' };
  }

  @Get('cards/:id')
  @ApiOperation({ summary: 'Get a single card by id' })
  @ApiParam({ name: 'id', type: Number })
  @ApiResponse({ status: 200, description: 'Card found' })
  async findOne(
    @Session() session: SessionData,
    @Param('id', ParseIntPipe) id: number,
  ): Promise<{ data: CardDetailResponse }> {
    const card = await this.cardsService.findById(id, session.userId);
    return { data: toCardDetailResponse(card) };
  }

  @Post('cards/:id/labels')
  @HttpCode(200)
  @ApiOperation({ summary: 'Assign a label to a card' })
  @ApiParam({ name: 'id', type: Number })
  @ApiResponse({ status: 200, description: 'Label assigned' })
  async assignLabel(
    @Session() session: SessionData,
    @Param('id', ParseIntPipe) id: number,
    @Body('labelId', ParseIntPipe) labelId: number,
  ): Promise<{ data: CardResponse; message: string }> {
    const card = await this.cardsService.assignLabel(id, labelId, session.userId);
    return { data: toCardResponse(card), message: 'Label assigned' };
  }

  @Delete('cards/:id/labels/:labelId')
  @ApiOperation({ summary: 'Remove a label from a card' })
  @ApiParam({ name: 'id', type: Number })
  @ApiParam({ name: 'labelId', type: Number })
  @ApiResponse({ status: 200, description: 'Label removed' })
  async removeLabel(
    @Session() session: SessionData,
    @Param('id', ParseIntPipe) id: number,
    @Param('labelId', ParseIntPipe) labelId: number,
  ): Promise<{ message: string }> {
    await this.cardsService.removeLabel(id, labelId, session.userId);
    return { message: 'Label removed' };
  }

  @Delete('cards/:id')
  @ApiOperation({ summary: 'Delete a card' })
  @ApiParam({ name: 'id', type: Number })
  @ApiResponse({ status: 200, description: 'Card deleted' })
  async remove(
    @Session() session: SessionData,
    @Param('id', ParseIntPipe) id: number,
  ): Promise<{ message: string }> {
    await this.cardsService.remove(id, session.userId);
    return { message: 'Card deleted' };
  }
}
