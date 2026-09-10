import { Test, TestingModule } from '@nestjs/testing';
import { getRepositoryToken } from '@nestjs/typeorm';
import { NotFoundException, ForbiddenException, ConflictException } from '@nestjs/common';
import { TagsService } from './tags.service';
import { Tag } from './entities/tag.entity';

describe('TagsService', () => {
  let service: TagsService;

  const mockUserId = 1;
  const mockDate = new Date('2026-01-01T00:00:00.000Z');

  const mockTag: Tag = {
    id: 1,
    name: 'important',
    color: 'teal',
    user_id: mockUserId,
    notes: [],
    created_at: mockDate,
  } as Tag;

  const mockTagRepository = {
    create: jest.fn(),
    save: jest.fn(),
    find: jest.fn(),
    findOne: jest.fn(),
    remove: jest.fn(),
    update: jest.fn(),
  };

  beforeEach(async () => {
    const module: TestingModule = await Test.createTestingModule({
      providers: [TagsService, { provide: getRepositoryToken(Tag), useValue: mockTagRepository }],
    }).compile();

    service = module.get<TagsService>(TagsService);
  });

  afterEach(() => {
    jest.clearAllMocks();
  });

  describe('create', () => {
    it('should create a tag with default color', async () => {
      mockTagRepository.findOne.mockResolvedValue(null);
      mockTagRepository.create.mockReturnValue({ ...mockTag, color: 'teal' });
      mockTagRepository.save.mockResolvedValue({ ...mockTag, color: 'teal' });

      const result = await service.create(mockUserId, { name: 'important' });

      expect(result).toBeDefined();
      expect(result.name).toBe('important');
      expect(result.color).toBe('teal');
      expect(mockTagRepository.create).toHaveBeenCalledWith({
        name: 'important',
        color: 'teal',
        user_id: mockUserId,
      });
      expect(mockTagRepository.save).toHaveBeenCalled();
    });

    it('should create a tag with provided color', async () => {
      mockTagRepository.findOne.mockResolvedValue(null);
      const tagWithColor = { ...mockTag, color: 'red' };
      mockTagRepository.create.mockReturnValue(tagWithColor);
      mockTagRepository.save.mockResolvedValue(tagWithColor);

      const result = await service.create(mockUserId, { name: 'urgent', color: 'red' });

      expect(result.color).toBe('red');
      expect(mockTagRepository.create).toHaveBeenCalledWith({
        name: 'urgent',
        color: 'red',
        user_id: mockUserId,
      });
    });

    it('should throw ConflictException if tag with same name exists', async () => {
      mockTagRepository.findOne.mockResolvedValue(mockTag);

      await expect(service.create(mockUserId, { name: 'important' })).rejects.toThrow(
        ConflictException,
      );
      expect(mockTagRepository.create).not.toHaveBeenCalled();
    });
  });

  describe('findAll', () => {
    it('should return all tags for user ordered by name', async () => {
      mockTagRepository.find.mockResolvedValue([mockTag]);

      const result = await service.findAll(mockUserId);

      expect(result).toHaveLength(1);
      expect(mockTagRepository.find).toHaveBeenCalledWith({
        where: { user_id: mockUserId },
        order: { name: 'ASC' },
      });
    });
  });

  describe('findOne', () => {
    it('should return a tag', async () => {
      mockTagRepository.findOne.mockResolvedValue(mockTag);

      const result = await service.findOne(1, mockUserId);

      expect(result).toBeDefined();
      expect(result.id).toBe(1);
    });

    it('should throw NotFoundException if tag does not exist', async () => {
      mockTagRepository.findOne.mockResolvedValue(null);

      await expect(service.findOne(999, mockUserId)).rejects.toThrow(NotFoundException);
    });

    it('should throw ForbiddenException if tag does not belong to user', async () => {
      mockTagRepository.findOne.mockResolvedValue({ ...mockTag, user_id: 999 });

      await expect(service.findOne(1, mockUserId)).rejects.toThrow(ForbiddenException);
    });
  });

  describe('update', () => {
    it('should update a tag name', async () => {
      mockTagRepository.findOne
        .mockResolvedValueOnce(mockTag)
        .mockResolvedValueOnce({ ...mockTag, name: 'urgent' });
      mockTagRepository.update.mockResolvedValue({ affected: 1 });

      const result = await service.update(1, mockUserId, { name: 'urgent' });

      expect(result.name).toBe('urgent');
      expect(mockTagRepository.update).toHaveBeenCalledWith(1, { name: 'urgent' });
    });

    it('should update a tag color', async () => {
      mockTagRepository.findOne
        .mockResolvedValueOnce(mockTag)
        .mockResolvedValueOnce({ ...mockTag, color: 'red' });
      mockTagRepository.update.mockResolvedValue({ affected: 1 });

      const result = await service.update(1, mockUserId, { color: 'red' });

      expect(result.color).toBe('red');
      expect(mockTagRepository.update).toHaveBeenCalledWith(1, { color: 'red' });
    });

    it('should throw NotFoundException if tag does not exist', async () => {
      mockTagRepository.findOne.mockResolvedValue(null);

      await expect(service.update(999, mockUserId, { name: 'New' })).rejects.toThrow(
        NotFoundException,
      );
    });
  });

  describe('remove', () => {
    it('should delete a tag', async () => {
      mockTagRepository.findOne.mockResolvedValue(mockTag);
      mockTagRepository.remove.mockResolvedValue(mockTag);

      await service.remove(1, mockUserId);

      expect(mockTagRepository.remove).toHaveBeenCalledWith(mockTag);
    });

    it('should throw NotFoundException if tag does not exist', async () => {
      mockTagRepository.findOne.mockResolvedValue(null);

      await expect(service.remove(999, mockUserId)).rejects.toThrow(NotFoundException);
    });
  });
});
