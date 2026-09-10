import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger';
import { IsString, IsNotEmpty, MaxLength, IsOptional, IsNumber, IsArray } from 'class-validator';

export class CreateNoteDto {
  @ApiProperty({ example: 'Meeting notes' })
  @IsNotEmpty()
  @IsString()
  @MaxLength(255)
  title!: string;

  @ApiProperty({ example: '# Markdown content' })
  @IsNotEmpty()
  @IsString()
  @MaxLength(65535)
  content!: string;

  @ApiPropertyOptional({ example: 1 })
  @IsOptional()
  @IsNumber()
  board_id?: number;

  @ApiPropertyOptional({ example: 1 })
  @IsOptional()
  @IsNumber()
  project_id?: number;

  @ApiPropertyOptional({ example: 1 })
  @IsOptional()
  @IsNumber()
  card_id?: number;

  @ApiPropertyOptional({ example: [1, 2] })
  @IsOptional()
  @IsArray()
  @IsNumber({}, { each: true })
  tagIds?: number[];
}
