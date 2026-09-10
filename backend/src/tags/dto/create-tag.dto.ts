import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger';
import { IsString, IsNotEmpty, MaxLength, IsOptional } from 'class-validator';

export class CreateTagDto {
  @ApiProperty({ example: 'important' })
  @IsNotEmpty()
  @IsString()
  @MaxLength(50)
  name!: string;

  @ApiPropertyOptional({ example: 'teal' })
  @IsOptional()
  @IsString()
  @MaxLength(20)
  color?: string;
}
