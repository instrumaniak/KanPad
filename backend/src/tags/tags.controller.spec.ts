import { Test, TestingModule } from '@nestjs/testing';
import { TagsController } from './tags.controller';
import { TagsService } from './tags.service';
import { Tag } from './entities/tag.entity';

describe('TagsController', () => {
  let controller: TagsController;

  const mockTagsService = {
    create: jest.fn(),
    findAll: jest.fn(),
    update: jest.fn(),
    remove: jest.fn(),
  };

  const mockDate = new Date('2026-01-01T00:00:00.000Z');

  const mockTag = {
    id: 1,
    name: 'important',
    color: 'teal',
    user_id: 1,
    created_at: mockDate,
  } as Tag;

  beforeEach(async () => {
    const module: TestingModule = await Test.createTestingModule({
      controllers: [TagsController],
      providers: [{ provide: TagsService, useValue: mockTagsService }],
    }).compile();

    controller = module.get<TagsController>(TagsController);
    jest.clearAllMocks();
  });

  describe('create', () => {
    it('should create a tag and return it with message', async () => {
      mockTagsService.create.mockResolvedValue(mockTag);

      const result = await controller.create({ userId: 1 }, { name: 'important', color: 'teal' });

      expect(result).toEqual({
        data: {
          id: 1,
          name: 'important',
          color: 'teal',
          user_id: 1,
          created_at: mockDate.toISOString(),
        },
        message: 'Tag created',
      });
      expect(mockTagsService.create).toHaveBeenCalledWith(1, {
        name: 'important',
        color: 'teal',
      });
    });
  });

  describe('findAll', () => {
    it('should return all tags for the user', async () => {
      mockTagsService.findAll.mockResolvedValue([mockTag]);

      const result = await controller.findAll({ userId: 1 });

      expect(result).toEqual({
        data: [
          {
            id: 1,
            name: 'important',
            color: 'teal',
            user_id: 1,
            created_at: mockDate.toISOString(),
          },
        ],
      });
      expect(mockTagsService.findAll).toHaveBeenCalledWith(1);
    });
  });

  describe('update', () => {
    it('should update a tag and return it with message', async () => {
      const updatedTag = { ...mockTag, name: 'urgent' } as Tag;
      mockTagsService.update.mockResolvedValue(updatedTag);

      const result = await controller.update({ userId: 1 }, 1, { name: 'urgent' });

      expect(result).toEqual({
        data: {
          id: 1,
          name: 'urgent',
          color: 'teal',
          user_id: 1,
          created_at: mockDate.toISOString(),
        },
        message: 'Tag updated',
      });
      expect(mockTagsService.update).toHaveBeenCalledWith(1, 1, { name: 'urgent' });
    });
  });

  describe('remove', () => {
    it('should delete a tag and return message', async () => {
      mockTagsService.remove.mockResolvedValue(undefined);

      const result = await controller.remove({ userId: 1 }, 1);

      expect(result).toEqual({ message: 'Tag deleted' });
      expect(mockTagsService.remove).toHaveBeenCalledWith(1, 1);
    });
  });
});
