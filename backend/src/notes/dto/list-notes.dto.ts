import { ApiPropertyOptional } from '@nestjs/swagger';
import { IsOptional, IsString, IsEnum, IsNumber } from 'class-validator';
import { Type } from 'class-transformer';

export enum NoteType {
  GENERAL = 'general',
  BOARD = 'board',
  PROJECT = 'project',
  CARD = 'card',
}

export class ListNotesDto {
  @ApiPropertyOptional({ example: 'meeting' })
  @IsOptional()
  @IsString()
  search?: string;

  @ApiPropertyOptional({ enum: NoteType, example: NoteType.BOARD })
  @IsOptional()
  @IsEnum(NoteType)
  type?: NoteType;

  @ApiPropertyOptional({ example: 1 })
  @IsOptional()
  @IsNumber()
  @Type(() => Number)
  tagId?: number;

  @ApiPropertyOptional({ example: 1 })
  @IsOptional()
  @IsNumber()
  @Type(() => Number)
  boardId?: number;

  @ApiPropertyOptional({ example: 1 })
  @IsOptional()
  @IsNumber()
  @Type(() => Number)
  page?: number;

  @ApiPropertyOptional({ example: 50 })
  @IsOptional()
  @IsNumber()
  @Type(() => Number)
  limit?: number;
}
