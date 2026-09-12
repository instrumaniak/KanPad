# KanPad

Self-hosted _project management & note taking_ tool for solo developers & small teams.

![KanPad screenshot](https://raziur.com/public_assets/projects/kanpad/KanPad-09-12-2026_opt.png)

### Why?

- Free & source-available (customize and extend as you want)
- No restrictions (create as many boards as you want)
- Self-hosted (deploy to your own server or even cPanel shared hosting!)
- Lightweight & fast

### Tech stack:

- _Frontend_: TypeScript, React, Vite
- _Backend_: TypeScript, Node.js, Nestjs, Fastify, TypeORM
- _Database_: MySQL

## Features

_Core_:

- [x] User authentication (registration, login, logout)
- [x] Board management (create, archive, restore)
- [x] Column management (create, sort, bulk move cards)
- [x] Card management (create, edit, delete, drag & drop)
- [x] Card detail panel with markdown descriptions
- [x] Color-coded labels
- [x] Card Due dates with calendar picker
- [x] Checklists with progress tracking
- [x] Dark mode support
- [x] Notes with markdown & Mermaid diagrams
- [ ] Projects (Organize boards & notes into projects)
- [x] Board view toggle (list/kanban)
- [x] Card search & filtering
- [ ] Admin panel (user management, registration toggle, activity log)
- [ ] Team collaboration (shared projects, boards, notes)
- [ ] Progressive web app (PWA) support + Responsive layout
- [ ] Finalize source code license

_Deployment_:

- [x] cPanel hosting
- [ ] Docker & templates for common self-hosting setup (portainer, dokploy)

## Backend setup

```bash
$ cd backend
```

First, make sure you have a MySQL database running and create a database for the application. Copy `.env.example` to `.env` and update the database connection details.

Install the packages and run the backend:

```bash
$ npm install

# migrate db
$ npm run migration:run

# development
$ npm run start

# watch mode
$ npm run start:dev
```

Run tests:

```bash
# unit tests
$ npm run test

# e2e tests
$ npm run test:e2e

# test coverage
$ npm run test:cov
```

## Frontend setup

```bash
$ cd frontend
```

Install the packages and run the frontend:

```bash
$ npm install

# development
$ npm run dev

```

Run tests:

```bash
# unit tests
$ npm run test

# e2e tests
$ npm run test:e2e

```

## Deployment

The backend will serve the frontend SPA/static files and the API.

```bash
$ cd backend
$ npm run build:release
```

This will build the production-ready version of the both **frontend** and **backend** and output it to the `release` directory in a ready to zip structure. Just Deploy the contents of the `release` directory accordingly.

From the release folder:
```bash
touch .env # create a .env file with your environment variables
node migrate.js # run database migrations
node app.js # start the backend server
```

### cPanel Hosting

The process is same for a typical nodejs application deployment on a cPanel shared hosting- Setup domain and create a nodejs application. Create your MySQL database. Upload the release build zip file to your cPanel hosting & extract the contents to your nodejs application directory.  Create `.env` and update the database connection details there. Run db migrations from terminal and start the server from nodejs application manager.

## Acknowledgement

### AI usage
AI assisted development tools were *heavily* used for planning & development of this project.
