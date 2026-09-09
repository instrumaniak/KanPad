import { MigrationInterface, QueryRunner } from 'typeorm';

export class CreateNotesTagsTables1779500000000 implements MigrationInterface {
  name = 'CreateNotesTagsTables1779500000000';

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      `CREATE TABLE IF NOT EXISTS \`notes\` (\`id\` int NOT NULL AUTO_INCREMENT, \`title\` varchar(255) NOT NULL, \`content\` mediumtext NOT NULL, \`board_id\` int NULL, \`project_id\` int NULL, \`card_id\` int NULL, \`user_id\` int NOT NULL, \`created_at\` datetime(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6), \`updated_at\` datetime(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6), PRIMARY KEY (\`id\`)) ENGINE=InnoDB`,
    );
    await queryRunner.query(
      `CREATE TABLE IF NOT EXISTS \`tags\` (\`id\` int NOT NULL AUTO_INCREMENT, \`name\` varchar(50) NOT NULL, \`color\` varchar(20) NOT NULL DEFAULT 'teal', \`user_id\` int NOT NULL, \`created_at\` datetime(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6), PRIMARY KEY (\`id\`)) ENGINE=InnoDB`,
    );
    await queryRunner.query(
      `CREATE TABLE IF NOT EXISTS \`note_tags\` (\`note_id\` int NOT NULL, \`tag_id\` int NOT NULL, PRIMARY KEY (\`note_id\`, \`tag_id\`)) ENGINE=InnoDB`,
    );
    await queryRunner.query(
      `ALTER TABLE \`notes\` ADD CONSTRAINT \`FK_notes_board_id\` FOREIGN KEY (\`board_id\`) REFERENCES \`boards\`(\`id\`) ON DELETE CASCADE ON UPDATE NO ACTION`,
    );
    await queryRunner.query(
      `ALTER TABLE \`notes\` ADD CONSTRAINT \`FK_notes_project_id\` FOREIGN KEY (\`project_id\`) REFERENCES \`projects\`(\`id\`) ON DELETE CASCADE ON UPDATE NO ACTION`,
    );
    await queryRunner.query(
      `ALTER TABLE \`notes\` ADD CONSTRAINT \`FK_notes_card_id\` FOREIGN KEY (\`card_id\`) REFERENCES \`cards\`(\`id\`) ON DELETE CASCADE ON UPDATE NO ACTION`,
    );
    await queryRunner.query(
      `ALTER TABLE \`notes\` ADD CONSTRAINT \`FK_notes_user_id\` FOREIGN KEY (\`user_id\`) REFERENCES \`users\`(\`id\`) ON DELETE CASCADE ON UPDATE NO ACTION`,
    );
    await queryRunner.query(
      `ALTER TABLE \`tags\` ADD CONSTRAINT \`FK_tags_user_id\` FOREIGN KEY (\`user_id\`) REFERENCES \`users\`(\`id\`) ON DELETE CASCADE ON UPDATE NO ACTION`,
    );
    await queryRunner.query(
      `ALTER TABLE \`note_tags\` ADD CONSTRAINT \`FK_note_tags_note_id\` FOREIGN KEY (\`note_id\`) REFERENCES \`notes\`(\`id\`) ON DELETE CASCADE ON UPDATE NO ACTION`,
    );
    await queryRunner.query(
      `ALTER TABLE \`note_tags\` ADD CONSTRAINT \`FK_note_tags_tag_id\` FOREIGN KEY (\`tag_id\`) REFERENCES \`tags\`(\`id\`) ON DELETE CASCADE ON UPDATE NO ACTION`,
    );
    await queryRunner.query(`CREATE INDEX \`IDX_notes_board_id\` ON \`notes\`(\`board_id\`)`);
    await queryRunner.query(`CREATE INDEX \`IDX_notes_user_id\` ON \`notes\`(\`user_id\`)`);
    await queryRunner.query(`CREATE INDEX \`IDX_tags_user_id\` ON \`tags\`(\`user_id\`)`);
    await queryRunner.query(`CREATE INDEX \`IDX_notes_project_id\` ON \`notes\`(\`project_id\`)`);
    await queryRunner.query(`CREATE INDEX \`IDX_notes_card_id\` ON \`notes\`(\`card_id\`)`);
    await queryRunner.query(`CREATE INDEX \`IDX_note_tags_tag_id\` ON \`note_tags\`(\`tag_id\`)`);
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(`ALTER TABLE \`note_tags\` DROP FOREIGN KEY \`FK_note_tags_tag_id\``);
    await queryRunner.query(`ALTER TABLE \`note_tags\` DROP FOREIGN KEY \`FK_note_tags_note_id\``);
    await queryRunner.query(`ALTER TABLE \`tags\` DROP FOREIGN KEY \`FK_tags_user_id\``);
    await queryRunner.query(`ALTER TABLE \`notes\` DROP FOREIGN KEY \`FK_notes_user_id\``);
    await queryRunner.query(`ALTER TABLE \`notes\` DROP FOREIGN KEY \`FK_notes_card_id\``);
    await queryRunner.query(`ALTER TABLE \`notes\` DROP FOREIGN KEY \`FK_notes_project_id\``);
    await queryRunner.query(`ALTER TABLE \`notes\` DROP FOREIGN KEY \`FK_notes_board_id\``);
    await queryRunner.query(`DROP INDEX \`IDX_tags_user_id\` ON \`tags\``);
    await queryRunner.query(`DROP INDEX \`IDX_notes_user_id\` ON \`notes\``);
    await queryRunner.query(`DROP INDEX \`IDX_notes_board_id\` ON \`notes\``);
    await queryRunner.query(`DROP INDEX \`IDX_note_tags_tag_id\` ON \`note_tags\``);
    await queryRunner.query(`DROP INDEX \`IDX_notes_card_id\` ON \`notes\``);
    await queryRunner.query(`DROP INDEX \`IDX_notes_project_id\` ON \`notes\``);
    await queryRunner.query(`DROP TABLE \`note_tags\``);
    await queryRunner.query(`DROP TABLE \`tags\``);
    await queryRunner.query(`DROP TABLE \`notes\``);
  }
}
