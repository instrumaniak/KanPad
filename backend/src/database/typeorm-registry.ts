import { User } from '../users/entities/user.entity';
import { Project } from '../projects/entities/project.entity';
import { Board } from '../boards/entities/board.entity';
import { BoardColumn } from '../columns/entities/column.entity';
import { Card } from '../cards/entities/card.entity';
import { CardLabel } from '../cards/entities/card-label.entity';
import { Label } from '../labels/entities/label.entity';
import { Session } from '../sessions/entities/session.entity';
import { Checklist } from '../checklists/entities/checklist.entity';
import { ChecklistItem } from '../checklists/entities/checklist-item.entity';
import { Note } from '../notes/entities/note.entity';
import { Tag } from '../tags/entities/tag.entity';

import { CreateUsersProjects1700000000000 } from '../migrations/1700000000000-CreateUsersProjects';
import { CreateBoardsColumns1745862000000 } from '../migrations/1745862000000-CreateBoardsColumns';
import { AddIsArchivedToBoards1777411200000 } from '../migrations/1777411200000-AddIsArchivedToBoards';
import { CreateCards1778000000000 } from '../migrations/1778000000000-CreateCards';
import { AddSessionsTable1778100000000 } from '../migrations/1778100000000-AddSessionsTable';
import { AddDescriptionAndDueDateToCards1778200000000 } from '../migrations/1778200000000-AddDescriptionAndDueDateToCards';
import { CreateLabelsAndCardLabelsTables1778300000000 } from '../migrations/1778300000000-CreateLabelsAndCardLabelsTables';
import { CreateChecklistsAndChecklistItems1779000000000 } from '../migrations/1779000000000-CreateChecklistsAndChecklistItems';
import { CreateNotesTagsTables1779500000000 } from '../migrations/1779500000000-CreateNotesTagsTables';

/**
 * Explicit registry of TypeORM entities.
 * Webpack cannot bundle dynamic glob patterns, so every entity
 * MUST be imported and registered here manually.
 */
export const entities = [
  User,
  Project,
  Board,
  BoardColumn,
  Card,
  CardLabel,
  Label,
  Session,
  Checklist,
  ChecklistItem,
  Note,
  Tag,
];

/**
 * Explicit registry of TypeORM migrations.
 * Every new migration MUST be imported and appended here manually.
 * Failure to register a migration will cause it to be silently skipped in the release bundle.
 */
export const migrations = [
  CreateUsersProjects1700000000000,
  CreateBoardsColumns1745862000000,
  AddIsArchivedToBoards1777411200000,
  CreateCards1778000000000,
  AddSessionsTable1778100000000,
  AddDescriptionAndDueDateToCards1778200000000,
  CreateLabelsAndCardLabelsTables1778300000000,
  CreateChecklistsAndChecklistItems1779000000000,
  CreateNotesTagsTables1779500000000,
];
