import {
  Injectable,
  NotFoundException,
  ForbiddenException,
  ConflictException,
} from '@nestjs/common';
import { InjectRepository } from '@nestjs/typeorm';
import { Repository } from 'typeorm';
import { Tag } from './entities/tag.entity';
import { CreateTagDto } from './dto/create-tag.dto';
import { UpdateTagDto } from './dto/update-tag.dto';

@Injectable()
export class TagsService {
  constructor(
    @InjectRepository(Tag)
    private readonly tagRepository: Repository<Tag>,
  ) {}

  async create(userId: number, dto: CreateTagDto): Promise<Tag> {
    const existing = await this.tagRepository.findOne({
      where: { name: dto.name, user_id: userId },
    });
    if (existing) {
      throw new ConflictException('Tag with this name already exists');
    }
    const tag = this.tagRepository.create({
      name: dto.name,
      color: dto.color ?? 'teal',
      user_id: userId,
    });
    return this.tagRepository.save(tag);
  }

  async findAll(userId: number): Promise<Tag[]> {
    return this.tagRepository.find({
      where: { user_id: userId },
      order: { name: 'ASC' },
    });
  }

  async findOne(id: number, userId: number): Promise<Tag> {
    const tag = await this.tagRepository.findOne({ where: { id } });
    if (!tag) {
      throw new NotFoundException('Tag not found');
    }
    if (tag.user_id !== userId) {
      throw new ForbiddenException('Access denied');
    }
    return tag;
  }

  async update(id: number, userId: number, dto: UpdateTagDto): Promise<Tag> {
    await this.findOne(id, userId);

    const updateData: Partial<Tag> = {};
    if (dto.name !== undefined) updateData.name = dto.name;
    if (dto.color !== undefined) updateData.color = dto.color;

    if (Object.keys(updateData).length > 0) {
      await this.tagRepository.update(id, updateData);
    }

    const updated = await this.tagRepository.findOne({ where: { id } });
    if (!updated) {
      throw new NotFoundException('Tag not found after update');
    }
    return updated;
  }

  async remove(id: number, userId: number): Promise<void> {
    const tag = await this.findOne(id, userId);
    await this.tagRepository.remove(tag);
  }
}
