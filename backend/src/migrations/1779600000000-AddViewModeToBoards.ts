import { MigrationInterface, QueryRunner } from 'typeorm';

export class AddViewModeToBoards1779600000000 implements MigrationInterface {
  name = 'AddViewModeToBoards1779600000000';

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      `ALTER TABLE \`boards\` ADD \`view_mode\` varchar(10) NOT NULL DEFAULT 'board'`,
    );
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(`ALTER TABLE \`boards\` DROP COLUMN \`view_mode\``);
  }
}
