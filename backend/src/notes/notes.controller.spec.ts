import { Test, TestingModule } from '@nestjs/testing';
import { NotesController } from './notes.controller';
import { NotesService } from './notes.service';
import { Note } from './entities/note.entity';

describe('NotesController', () => {
  let controller: NotesController;

  const mockNotesService = {
    create: jest.fn(),
    findAll: jest.fn(),
    findById: jest.fn(),
    findByBoardId: jest.fn(),
    update: jest.fn(),
    remove: jest.fn(),
  };

  const mockDate = new Date('2026-01-01T00:00:00.000Z');

  const mockNote = {
    id: 1,
    title: 'Meeting notes',
    content: '# Markdown content',
    board_id: 1,
    project_id: null,
    card_id: null,
    user_id: 1,
    tags: [{ id: 1, name: 'important', color: 'teal' }],
    created_at: mockDate,
    updated_at: mockDate,
  } as unknown as Note;

  beforeEach(async () => {
    const module: TestingModule = await Test.createTestingModule({
      controllers: [NotesController],
      providers: [{ provide: NotesService, useValue: mockNotesService }],
    }).compile();

    controller = module.get<NotesController>(NotesController);
    jest.clearAllMocks();
  });

  describe('create', () => {
    it('should create a note and return it with message', async () => {
      mockNotesService.create.mockResolvedValue(mockNote);

      const result = await controller.create(
        { userId: 1 },
        { title: 'Meeting notes', content: '# Markdown content', board_id: 1 },
      );

      expect(result).toEqual({
        data: {
          id: 1,
          title: 'Meeting notes',
          content: '# Markdown content',
          board_id: 1,
          project_id: null,
          card_id: null,
          user_id: 1,
          tags: [{ id: 1, name: 'important', color: 'teal' }],
          created_at: mockDate.toISOString(),
          updated_at: mockDate.toISOString(),
        },
        message: 'Note created',
      });
      expect(mockNotesService.create).toHaveBeenCalledWith(1, {
        title: 'Meeting notes',
        content: '# Markdown content',
        board_id: 1,
      });
    });
  });

  describe('findAll', () => {
    it('should return all notes', async () => {
      mockNotesService.findAll.mockResolvedValue([mockNote]);

      const result = await controller.findAll({ userId: 1 }, {});

      expect(result).toEqual({
        data: [
          {
            id: 1,
            title: 'Meeting notes',
            content: '# Markdown content',
            board_id: 1,
            project_id: null,
            card_id: null,
            user_id: 1,
            tags: [{ id: 1, name: 'important', color: 'teal' }],
            created_at: mockDate.toISOString(),
            updated_at: mockDate.toISOString(),
          },
        ],
      });
      expect(mockNotesService.findAll).toHaveBeenCalledWith(1, {});
    });
  });

  describe('findOne', () => {
    it('should return a single note by id', async () => {
      mockNotesService.findById.mockResolvedValue(mockNote);

      const result = await controller.findOne({ userId: 1 }, 1);

      expect(result).toEqual({
        data: {
          id: 1,
          title: 'Meeting notes',
          content: '# Markdown content',
          board_id: 1,
          project_id: null,
          card_id: null,
          user_id: 1,
          tags: [{ id: 1, name: 'important', color: 'teal' }],
          created_at: mockDate.toISOString(),
          updated_at: mockDate.toISOString(),
        },
      });
      expect(mockNotesService.findById).toHaveBeenCalledWith(1, 1);
    });
  });

  describe('update', () => {
    it('should update a note and return it with message', async () => {
      const updatedNote = { ...mockNote, title: 'Updated title' } as unknown as Note;
      mockNotesService.update.mockResolvedValue(updatedNote);

      const result = await controller.update({ userId: 1 }, 1, { title: 'Updated title' });

      expect(result).toEqual({
        data: {
          id: 1,
          title: 'Updated title',
          content: '# Markdown content',
          board_id: 1,
          project_id: null,
          card_id: null,
          user_id: 1,
          tags: [{ id: 1, name: 'important', color: 'teal' }],
          created_at: mockDate.toISOString(),
          updated_at: mockDate.toISOString(),
        },
        message: 'Note updated',
      });
      expect(mockNotesService.update).toHaveBeenCalledWith(1, 1, { title: 'Updated title' });
    });
  });

  describe('remove', () => {
    it('should delete a note and return message', async () => {
      mockNotesService.remove.mockResolvedValue(undefined);

      const result = await controller.remove({ userId: 1 }, 1);

      expect(result).toEqual({ message: 'Note deleted' });
      expect(mockNotesService.remove).toHaveBeenCalledWith(1, 1);
    });
  });

  describe('findByBoardId', () => {
    it('should return notes for a board', async () => {
      mockNotesService.findByBoardId.mockResolvedValue([mockNote]);

      const result = await controller.findByBoardId({ userId: 1 }, 1);

      expect(result).toEqual({
        data: [
          {
            id: 1,
            title: 'Meeting notes',
            content: '# Markdown content',
            board_id: 1,
            project_id: null,
            card_id: null,
            user_id: 1,
            tags: [{ id: 1, name: 'important', color: 'teal' }],
            created_at: mockDate.toISOString(),
            updated_at: mockDate.toISOString(),
          },
        ],
      });
      expect(mockNotesService.findByBoardId).toHaveBeenCalledWith(1, 1);
    });
  });
});
