import { ApiPropertyOptional } from '@nestjs/swagger';
import { IsString, MaxLength, IsOptional } from 'class-validator';

export class UpdateTagDto {
  @ApiPropertyOptional({ example: 'urgent' })
  @IsOptional()
  @IsString()
  @MaxLength(50)
  name?: string;

  @ApiPropertyOptional({ example: 'red' })
  @IsOptional()
  @IsString()
  @MaxLength(20)
  color?: string;
}
