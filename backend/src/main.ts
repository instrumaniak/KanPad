import fs from 'node:fs';
import { join } from 'node:path';
import { NestFactory } from '@nestjs/core';
import { DocumentBuilder, SwaggerModule } from '@nestjs/swagger';
import { AppModule } from './app.module';
import { DataSource } from 'typeorm';
import { Session } from './sessions/entities/session.entity';
import { TypeormStore } from 'connect-typeorm';
import { FastifyAdapter, NestFastifyApplication } from '@nestjs/platform-fastify';
import type { FastifyReply, FastifyRequest } from 'fastify';
import fastifyCompress from '@fastify/compress';
import fastifyHelmet from '@fastify/helmet';
import fastifyCookie from '@fastify/cookie';
import fastifySession from '@fastify/session';
import fastifyStatic from '@fastify/static';

async function bootstrap() {
  const app = await NestFactory.create<NestFastifyApplication>(
    AppModule,
    new FastifyAdapter({
      trustProxy: true,
    }),
  );

  // --- Session store setup (must be before app.init() for routes to have session) ---
  const dataSource = app.get(DataSource);
  const sessionRepository = dataSource.getRepository(Session);

  await app.register(fastifyCookie);

  await app.register(fastifySession, {
    store: new TypeormStore({
      ttl: 86400,
      cleanupLimit: 10,
      limitSubquery: false,
      onError: (store, error: Error) => console.error('Session store error:', error),
    }).connect(sessionRepository),
    secret: getSessionSecret(),
    cookieName: 'connect.sid',
    cookie: {
      httpOnly: true,
      sameSite: 'lax',
      secure: process.env.NODE_ENV === 'production',
      maxAge: 86400000,
    },
  });

  await app.register(fastifyHelmet, {
    contentSecurityPolicy: false,
  });

  await app.register(fastifyCompress, {
    global: true,
    threshold: 1024,
  });

  const config = new DocumentBuilder()
    .setTitle('KanPad API')
    .setVersion('0.0.1')
    .addCookieAuth('connect.sid', {
      type: 'apiKey',
      description: 'Session cookie — login first via POST /api/auth/login',
    })
    .build();
  const document = SwaggerModule.createDocument(app, config);
  SwaggerModule.setup('api/docs', app, document);

  // --- Static file serving + SPA fallback (release only) ---
  const publicPath = join(__dirname, 'public');
  const hasPublicFolder = fs.existsSync(publicPath);

  if (hasPublicFolder) {
    await app.register(fastifyStatic, {
      root: publicPath,
      wildcard: false,
    });

    // SPA fallback: serve index.html for all non-API routes
    const fastify = app.getHttpAdapter().getInstance();
    fastify.get('*', (req: FastifyRequest, res: FastifyReply) => {
      if (req.url?.startsWith('/api/')) {
        res.code(404).send({ message: 'Not Found' });
        return;
      }
      res.sendFile('index.html', publicPath);
    });
  } else {
    console.log('[INFO] No public/ folder found - SPA fallback disabled');
  }

  await app.listen({ port: Number(process.env.PORT ?? 3000), host: '0.0.0.0' });
}

function getSessionSecret(): string {
  const secret = process.env.SESSION_SECRET;
  if (!secret) {
    throw new Error('SESSION_SECRET environment variable is required');
  }
  return secret;
}

void bootstrap();
