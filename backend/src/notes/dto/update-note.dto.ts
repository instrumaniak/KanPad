import { ApiPropertyOptional } from '@nestjs/swagger';
import { Transform } from 'class-transformer';
import { IsString, MaxLength, IsOptional, IsNumber, IsArray } from 'class-validator';

export class UpdateNoteDto {
  @ApiPropertyOptional({ example: 'Updated title' })
  @IsOptional()
  @IsString()
  @MaxLength(255)
  title?: string;

  @ApiPropertyOptional({ example: '# Updated content' })
  @IsOptional()
  @IsString()
  @MaxLength(65535)
  content?: string;

  @ApiPropertyOptional({ example: 1 })
  @IsOptional()
  @Transform(({ value }) =>
    value === null ? null : value === undefined ? undefined : Number(value),
  )
  board_id?: number | null;

  @ApiPropertyOptional({ example: 1 })
  @IsOptional()
  @Transform(({ value }) =>
    value === null ? null : value === undefined ? undefined : Number(value),
  )
  project_id?: number | null;

  @ApiPropertyOptional({ example: 1 })
  @IsOptional()
  @Transform(({ value }) =>
    value === null ? null : value === undefined ? undefined : Number(value),
  )
  card_id?: number | null;

  @ApiPropertyOptional({ example: [1, 2] })
  @IsOptional()
  @IsArray()
  @IsNumber({}, { each: true })
  tagIds?: number[];
}
