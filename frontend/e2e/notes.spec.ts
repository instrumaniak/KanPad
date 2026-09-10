import { test, expect } from '@playwright/test';

const TEST_EMAIL = 'e2e-notes@test.local';
const TEST_PASSWORD = 'Test1234!';

test.describe('Notes Feature', () => {
  let boardId: number;

  test.beforeAll(async ({ request }) => {
    const res = await request.post('http://localhost:3000/api/auth/register', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    if (res.status() !== 201 && res.status() !== 409) {
      throw new Error(`Failed to create test user: ${res.status()}`);
    }

    const loginRes = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';

    const boardRes = await request.post('http://localhost:3000/api/boards', {
      data: { name: `Notes Test Board ${Date.now()}` },
      headers: { Cookie: cookies },
    });
    boardId = (await boardRes.json()).data.id;
  });

  test('creates a board note via sidebar', async ({ page, request }) => {
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto(`/board/${boardId}`);
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    await page.locator('[aria-label="Show notes sidebar"]').click();
    await page.waitForTimeout(300);

    await page.getByRole('button', { name: 'New Note' }).click();

    await expect(page.getByRole('dialog')).toBeVisible({ timeout: 5000 });
    await expect(page.getByPlaceholder('Note title...')).toBeVisible({ timeout: 5000 });

    const noteTitle = `Board Note ${Date.now()}`;
    await page.getByPlaceholder('Note title...').fill(noteTitle);
    await page.getByPlaceholder('Write your note content in markdown...').fill('# Hello');

    const createResponse = page.waitForResponse(
      (resp) => resp.url().includes('/api/notes') && resp.request().method() === 'POST',
    );
    await page.getByRole('button', { name: 'Save' }).click();
    const createResp = await createResponse;
    expect(createResp.status()).toBe(201);

    await expect(page.getByPlaceholder('Note title...')).not.toBeVisible({ timeout: 5000 });
    await page.waitForTimeout(500);

    const noteArticle = page.getByRole('article', { name: new RegExp(`Note: ${noteTitle}`, 'i') });
    await expect(noteArticle).toBeVisible({ timeout: 5000 });

    await page.reload();
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    await page.locator('[aria-label="Show notes sidebar"]').click();
    await page.waitForTimeout(300);

    await expect(noteArticle).toBeVisible({ timeout: 5000 });

    const loginRes = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';
    const notesRes = await request.get(`http://localhost:3000/api/boards/${boardId}/notes`, {
      headers: { Cookie: cookies },
    });
    expect(notesRes.status()).toBe(200);
    const notesData = await notesRes.json();
    const createdNote = notesData.data.find((n: { title: string }) => n.title === noteTitle);
    expect(createdNote).toBeDefined();
    expect(createdNote.board_id).toBe(boardId);
  });

  test('creates a standalone note via /notes page', async ({ page, request }) => {
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto('/notes');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    await page.getByRole('button', { name: 'New Note' }).click();

    await expect(page.getByRole('dialog')).toBeVisible({ timeout: 5000 });
    await expect(page.getByText('Create Note')).toBeVisible({ timeout: 5000 });
    await expect(page.getByPlaceholder('Note title...')).toBeVisible({ timeout: 5000 });

    const noteTitle = `Standalone Note ${Date.now()}`;
    await page.getByPlaceholder('Note title...').fill(noteTitle);
    await page.getByPlaceholder('Write your note content in markdown...').fill('Plain text content');

    const createResponse = page.waitForResponse(
      (resp) => resp.url().includes('/api/notes') && resp.request().method() === 'POST',
    );
    await page.getByRole('button', { name: 'Save' }).click();
    const createResp = await createResponse;
    expect(createResp.status()).toBe(201);
    const noteData = await createResp.json();
    const noteId = noteData.data.id;

    await expect(page.getByPlaceholder('Note title...')).not.toBeVisible({ timeout: 5000 });
    await page.waitForTimeout(500);

    const noteArticle = page.getByRole('article', { name: new RegExp(`Note: ${noteTitle}`, 'i') });
    await expect(noteArticle).toBeVisible({ timeout: 5000 });
    await expect(noteArticle.getByText('general')).toBeVisible();

    await page.reload();
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    await expect(noteArticle).toBeVisible({ timeout: 5000 });
    await expect(noteArticle.getByText('general')).toBeVisible();

    const loginRes = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';
    const noteRes = await request.get(`http://localhost:3000/api/notes/${noteId}`, {
      headers: { Cookie: cookies },
    });
    expect(noteRes.status()).toBe(200);
    const fetchedNote = await noteRes.json();
    expect(fetchedNote.data.title).toBe(noteTitle);
    expect(fetchedNote.data.board_id).toBeNull();
    expect(fetchedNote.data.project_id).toBeNull();
    expect(fetchedNote.data.card_id).toBeNull();
  });

  test('renders markdown and edits a note', async ({ page, request }) => {
    const loginRes = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';

    const noteTitle = `Markdown Edit ${Date.now()}`;
    const createRes = await request.post('http://localhost:3000/api/notes', {
      data: { title: noteTitle, content: '# Hello\nThis is **bold** and `code`' },
      headers: { Cookie: cookies },
    });
    expect(createRes.status()).toBe(201);
    const noteId = (await createRes.json()).data.id;

    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto('/notes');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    await page.getByText(noteTitle).click();
    await page.waitForTimeout(500);

    await expect(page.getByRole('heading', { name: 'Hello' })).toBeVisible();
    await expect(page.locator('strong').first()).toHaveText('bold');
    await expect(page.locator('code').first()).toHaveText('code');

    await page.getByRole('button', { name: 'Edit' }).click();

    await expect(page.getByPlaceholder('Note title...')).toBeVisible({ timeout: 5000 });

    const updatedTitle = `Updated Title ${Date.now()}`;
    await page.getByPlaceholder('Note title...').clear();
    await page.getByPlaceholder('Note title...').fill(updatedTitle);
    await page.getByPlaceholder('Write your note content in markdown...').clear();
    await page.getByPlaceholder('Write your note content in markdown...').fill('Updated **content**');

    const editResponse = page.waitForResponse(
      (resp) => resp.url().includes('/api/notes/') && resp.request().method() === 'PATCH',
    );
    await page.getByRole('button', { name: 'Save' }).click();
    const editResp = await editResponse;
    expect(editResp.status()).toBe(200);

    await expect(page.getByText('Note saved')).toBeVisible({ timeout: 10000 });
    await page.waitForTimeout(500);

    await expect(page.getByText(updatedTitle)).toBeVisible({ timeout: 5000 });

    await page.reload();
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    await expect(page.getByText(updatedTitle)).toBeVisible({ timeout: 5000 });

    const loginRes2 = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const c2 = loginRes2.headers()['set-cookie'] || '';
    const noteRes = await request.get(`http://localhost:3000/api/notes/${noteId}`, {
      headers: { Cookie: c2 },
    });
    expect(noteRes.status()).toBe(200);
    const fetchedNote = await noteRes.json();
    expect(fetchedNote.data.title).toBe(updatedTitle);
    expect(fetchedNote.data.content).toBe('Updated **content**');
  });

  test('deletes a note with confirmation', async ({ page, request }) => {
    const loginRes = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';

    const noteTitle = `Delete Test ${Date.now()}`;
    const createRes = await request.post('http://localhost:3000/api/notes', {
      data: { title: noteTitle, content: 'To be deleted' },
      headers: { Cookie: cookies },
    });
    expect(createRes.status()).toBe(201);
    const noteId = (await createRes.json()).data.id;

    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto('/notes');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    await page.getByText(noteTitle).first().click();

    await expect(page.getByRole('button', { name: 'Edit' })).toBeVisible({ timeout: 5000 });

    await page.getByRole('button', { name: 'Delete' }).click();
    await expect(page.getByRole('heading', { name: 'Delete note?' })).toBeVisible({ timeout: 5000 });

    await page.getByRole('button', { name: 'Delete' }).click();
    await expect(page.getByRole('heading', { name: 'Delete note?' })).toBeVisible({ timeout: 5000 });

    const deleteResponse = page.waitForResponse(
      (resp) => resp.url().includes('/api/notes/') && resp.request().method() === 'DELETE',
    );
    await page.getByRole('button', { name: 'Delete' }).click();
    const deleteResp = await deleteResponse;
    expect(deleteResp.status()).toBe(200);

    await expect(page.getByText(noteTitle).first()).not.toBeVisible({ timeout: 5000 });

    await page.reload();
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    await expect(page.getByText(noteTitle).first()).not.toBeVisible();

    const loginRes2 = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const c2 = loginRes2.headers()['set-cookie'] || '';
    const deletedNoteRes = await request.get(`http://localhost:3000/api/notes/${noteId}`, {
      headers: { Cookie: c2 },
    });
    expect(deletedNoteRes.status()).toBe(404);
  });

  test('creates a note with tags', async ({ page, request }) => {
    const loginRes = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';

    const tagName = `Tag ${Date.now()}`;
    const tagRes = await request.post('http://localhost:3000/api/tags', {
      data: { name: tagName, color: 'blue' },
      headers: { Cookie: cookies },
    });
    expect(tagRes.status()).toBe(201);

    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto('/notes');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    await page.getByRole('button', { name: 'New Note' }).click();

    await expect(page.getByPlaceholder('Note title...')).toBeVisible({ timeout: 5000 });

    const noteTitle = `Tagged Note ${Date.now()}`;
    await page.getByPlaceholder('Note title...').fill(noteTitle);
    await page.getByPlaceholder('Write your note content in markdown...').fill('Note with tags');

    await page.getByText('Add tags').click();
    await page.waitForTimeout(300);

    await expect(page.getByPlaceholder('Search tags...')).toBeVisible();
    await page.getByRole('button', { name: tagName }).click();
    await page.waitForTimeout(300);

    await expect(page.getByText(tagName)).toBeVisible();

    await page.getByText('Add tags').click();
    await page.waitForTimeout(300);

    const createResponse = page.waitForResponse(
      (resp) => resp.url().includes('/api/notes') && resp.request().method() === 'POST',
    );
    await page.getByRole('button', { name: 'Save' }).click();
    const createResp = await createResponse;
    expect(createResp.status()).toBe(201);
    const noteData = await createResp.json();
    const noteId = noteData.data.id;

    await expect(page.getByPlaceholder('Note title...')).not.toBeVisible({ timeout: 5000 });
    await page.waitForTimeout(500);

    await expect(page.getByText(noteTitle)).toBeVisible({ timeout: 5000 });

    await page.reload();
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    await expect(page.getByText(noteTitle)).toBeVisible({ timeout: 5000 });

    const loginRes2 = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const c2 = loginRes2.headers()['set-cookie'] || '';
    const noteRes = await request.get(`http://localhost:3000/api/notes/${noteId}`, {
      headers: { Cookie: c2 },
    });
    expect(noteRes.status()).toBe(200);
    const fetchedNote = await noteRes.json();
    expect(fetchedNote.data.tags).toBeDefined();
    expect(fetchedNote.data.tags.length).toBe(1);
    expect(fetchedNote.data.tags[0].name).toBe(tagName);
  });

  test('shows empty sidebar when board has no notes', async ({ page, request }) => {
    const loginRes = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';

    const emptyBoardRes = await request.post('http://localhost:3000/api/boards', {
      data: { name: `Empty Board ${Date.now()}` },
      headers: { Cookie: cookies },
    });
    expect(emptyBoardRes.status()).toBe(201);
    const emptyBoardData = await emptyBoardRes.json();
    const emptyBoardId = emptyBoardData.data.id;

    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto(`/board/${emptyBoardId}`);
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    const sidebarBtn = page.locator('[aria-label="Show notes sidebar"]');
    if (await sidebarBtn.isVisible()) {
      await sidebarBtn.click();
      await page.waitForTimeout(300);
    }

    await expect(page.getByText('No notes for this board')).toBeVisible({ timeout: 5000 });

    await expect(page.getByRole('button', { name: 'New Note' })).toBeVisible();

    await page.reload();
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    if (await sidebarBtn.isVisible()) {
      await sidebarBtn.click();
      await page.waitForTimeout(300);
    }

    await expect(page.getByText('No notes for this board')).toBeVisible({ timeout: 5000 });
  });

  test('sidebar is hidden on /notes page and visible on board page', async ({ page }) => {
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto('/notes');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    await expect(page.locator('[aria-label="Show notes sidebar"]')).not.toBeVisible();
    await expect(page.locator('[aria-label="Toggle notes sidebar"]')).not.toBeVisible();

    await page.goto(`/board/${boardId}`);
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    const sidebarToggle = page.locator('[aria-label="Show notes sidebar"]');
    await expect(sidebarToggle).toBeVisible({ timeout: 5000 });
  });

  test('collapses and expands notes sidebar', async ({ page }) => {
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto(`/board/${boardId}`);
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    await page.locator('[aria-label="Show notes sidebar"]').click();
    await page.waitForTimeout(300);

    await expect(page.locator('[aria-label="Toggle notes sidebar"]')).toBeVisible();

    await page.locator('[aria-label="Toggle notes sidebar"]').click();
    await page.waitForTimeout(300);

    await expect(page.locator('[aria-label="Show notes sidebar"]')).toBeVisible();

    await page.locator('[aria-label="Show notes sidebar"]').click();
    await page.waitForTimeout(300);

    await expect(page.locator('[aria-label="Toggle notes sidebar"]')).toBeVisible();
  });

  test('search by title filters notes list', async ({ page, request }) => {
    const loginRes = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';

    const titleA = `Search Alpha ${Date.now()}`;
    const titleB = `Search Beta ${Date.now()}`;

    const resA = await request.post('http://localhost:3000/api/notes', {
      data: { title: titleA, content: 'First note' },
      headers: { Cookie: cookies },
    });
    expect(resA.status()).toBe(201);
    const noteA = await resA.json();
    const noteIdA = noteA.data.id;

    const resB = await request.post('http://localhost:3000/api/notes', {
      data: { title: titleB, content: 'Second note' },
      headers: { Cookie: cookies },
    });
    expect(resB.status()).toBe(201);
    const noteB = await resB.json();
    const noteIdB = noteB.data.id;

    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto('/notes');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    await expect(page.getByRole('article', { name: new RegExp(`Note: ${titleA}`, 'i') })).toBeVisible();
    await expect(page.getByRole('article', { name: new RegExp(`Note: ${titleB}`, 'i') })).toBeVisible();

    await page.getByPlaceholder('Search notes...').fill('Alpha');
    await page.waitForTimeout(500);
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(300);

    await expect(page.getByRole('article', { name: new RegExp(`Note: ${titleA}`, 'i') })).toBeVisible();
    const betaArticle = page.getByRole('article', { name: new RegExp(`Note: ${titleB}`, 'i') });
    await expect(betaArticle).not.toBeVisible();

    await page.getByPlaceholder('Search notes...').clear();
    await page.waitForTimeout(500);
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(300);

    await expect(page.getByRole('article', { name: new RegExp(`Note: ${titleA}`, 'i') })).toBeVisible();
    await expect(page.getByRole('article', { name: new RegExp(`Note: ${titleB}`, 'i') })).toBeVisible();

    const loginRes2 = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const c2 = loginRes2.headers()['set-cookie'] || '';
    const checkA = await request.get(`http://localhost:3000/api/notes/${noteIdA}`, { headers: { Cookie: c2 } });
    expect(checkA.status()).toBe(200);
    const checkB = await request.get(`http://localhost:3000/api/notes/${noteIdB}`, { headers: { Cookie: c2 } });
    expect(checkB.status()).toBe(200);
  });

  test('type filter chips filter notes by type', async ({ page, request }) => {
    const loginRes = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';

    const generalTitle = `General Note ${Date.now()}`;
    const boardTitle = `Board Note TypeFilter ${Date.now()}`;

    const genRes = await request.post('http://localhost:3000/api/notes', {
      data: { title: generalTitle, content: 'General content' },
      headers: { Cookie: cookies },
    });
    expect(genRes.status()).toBe(201);
    const genData = await genRes.json();
    const genId = genData.data.id;

    const boardRes = await request.post('http://localhost:3000/api/notes', {
      data: { title: boardTitle, content: 'Board content', board_id: boardId },
      headers: { Cookie: cookies },
    });
    expect(boardRes.status()).toBe(201);
    const boardData = await boardRes.json();
    const boardNoteId = boardData.data.id;

    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto('/notes');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    const generalArticle = page.getByRole('article', { name: new RegExp(`Note: ${generalTitle}`, 'i') });
    const boardArticle = page.getByRole('article', { name: new RegExp(`Note: ${boardTitle}`, 'i') });
    await expect(generalArticle).toBeVisible();
    await expect(boardArticle).toBeVisible();

    await page.getByText('Board').first().click();
    await page.waitForTimeout(500);
    await page.waitForLoadState('networkidle');

    await expect(boardArticle).toBeVisible({ timeout: 5000 });
    await expect(generalArticle).not.toBeVisible();

    await page.getByText('General').first().click();
    await page.waitForTimeout(500);
    await page.waitForLoadState('networkidle');

    await expect(generalArticle).toBeVisible({ timeout: 5000 });
    await expect(boardArticle).not.toBeVisible();

    await page.getByText('All').first().click();
    await page.waitForTimeout(500);
    await page.waitForLoadState('networkidle');

    await expect(generalArticle).toBeVisible({ timeout: 5000 });
    await expect(boardArticle).toBeVisible({ timeout: 5000 });

    const loginRes2 = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const c2 = loginRes2.headers()['set-cookie'] || '';
    const checkGen = await request.get(`http://localhost:3000/api/notes/${genId}`, { headers: { Cookie: c2 } });
    expect(checkGen.status()).toBe(200);
    const checkBoard = await request.get(`http://localhost:3000/api/notes/${boardNoteId}`, { headers: { Cookie: c2 } });
    expect(checkBoard.status()).toBe(200);
  });

  test('tag colored badge renders with correct color', async ({ page, request }) => {
    const loginRes = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';

    const tagName = `ColorTag ${Date.now()}`;
    const tagRes = await request.post('http://localhost:3000/api/tags', {
      data: { name: tagName, color: 'teal' },
      headers: { Cookie: cookies },
    });
    expect(tagRes.status()).toBe(201);
    const tagData = await tagRes.json();
    const tagId = tagData.data.id;

    const noteTitle = `Tag Color ${Date.now()}`;
    const noteRes = await request.post('http://localhost:3000/api/notes', {
      data: { title: noteTitle, content: 'Tag color test', tagIds: [tagId] },
      headers: { Cookie: cookies },
    });
    expect(noteRes.status()).toBe(201);
    const noteData = await noteRes.json();
    const noteId = noteData.data.id;

    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto('/notes');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    const noteArticle = page.getByRole('article', { name: new RegExp(`Note: ${noteTitle}`, 'i') });
    await expect(noteArticle).toBeVisible({ timeout: 5000 });

    const tagBadge = noteArticle.locator('[class*="cursor-"]').filter({ hasText: tagName }).first();
    await expect(tagBadge).toBeVisible();

    const styleAttr = await tagBadge.getAttribute('style');
    expect(styleAttr).not.toBeNull();
    expect(styleAttr!.toLowerCase()).toContain('teal');

    await page.reload();
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    const noteArticleReload = page.getByRole('article', { name: new RegExp(`Note: ${noteTitle}`, 'i') });
    await expect(noteArticleReload).toBeVisible({ timeout: 5000 });
    const tagBadgeReload = noteArticleReload.locator('[class*="cursor-"]').filter({ hasText: tagName }).first();
    await expect(tagBadgeReload).toBeVisible();
    const styleAttrReload = await tagBadgeReload.getAttribute('style');
    expect(styleAttrReload).not.toBeNull();
    expect(styleAttrReload!.toLowerCase()).toContain('teal');

    const loginRes2 = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const c2 = loginRes2.headers()['set-cookie'] || '';
    const checkRes = await request.get(`http://localhost:3000/api/notes/${noteId}`, { headers: { Cookie: c2 } });
    expect(checkRes.status()).toBe(200);
    const checkData = await checkRes.json();
    expect(checkData.data.tags).toBeDefined();
    expect(checkData.data.tags.length).toBe(1);
    expect(checkData.data.tags[0].color).toBe('teal');
  });

  test('tag click filters notes by that tag', async ({ page, request }) => {
    const ts = Date.now();
    const loginRes = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';

    const tagRes = await request.post('http://localhost:3000/api/tags', {
      data: { name: `FilterTagClick ${ts}`, color: 'teal' },
      headers: { Cookie: cookies },
    });
    expect(tagRes.status()).toBe(201);
    const tagId = (await tagRes.json()).data.id;

    const taggedTitle = `Tagged Note Filter ${ts}`;
    const untaggedTitle = `Untagged Note Filter ${ts}`;

    await request.post('http://localhost:3000/api/notes', {
      data: { title: taggedTitle, content: 'Has tag', tagIds: [tagId] },
      headers: { Cookie: cookies },
    });

    await request.post('http://localhost:3000/api/notes', {
      data: { title: untaggedTitle, content: 'No tag' },
      headers: { Cookie: cookies },
    });

    const loginRes2 = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const c2 = loginRes2.headers()['set-cookie'] || '';
    const allRes = await request.get('http://localhost:3000/api/notes', { headers: { Cookie: c2 } });
    expect(allRes.status()).toBe(200);
    const allNotes: { data: { id: number; title: string; tags?: { id: number }[] }[] } = await allRes.json();
    const taggedInDb = allNotes.data.find((n) => n.title === taggedTitle);
    expect(taggedInDb).toBeDefined();
    expect(taggedInDb!.tags?.length).toBe(1);
    expect(taggedInDb!.tags![0].id).toBe(tagId);

    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto('/notes');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    const taggedItem = page.getByText(taggedTitle).first();
    const untaggedItem = page.getByText(untaggedTitle).first();

    await expect(taggedItem).toBeVisible();
    await expect(untaggedItem).toBeVisible();

    const tagBadge = page.locator('[class*="hover:opacity-80"]').filter({ hasText: new RegExp(`FilterTagClick ${ts}`) });
    await expect(tagBadge).toBeVisible();
    await tagBadge.click();
    await page.waitForTimeout(1000);

    await expect(taggedItem).toBeVisible({ timeout: 5000 });
    await expect(untaggedItem).not.toBeVisible();

    await tagBadge.click();
    await page.waitForTimeout(1000);

    await expect(taggedItem).toBeVisible({ timeout: 5000 });
    await expect(untaggedItem).toBeVisible({ timeout: 5000 });
  });

  test('undo delete keeps the note', async ({ page, request }) => {
    const loginRes = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';

    const noteTitle = `Undo Test ${Date.now()}`;
    const createRes = await request.post('http://localhost:3000/api/notes', {
      data: { title: noteTitle, content: 'Undo me' },
      headers: { Cookie: cookies },
    });
    expect(createRes.status()).toBe(201);
    const noteId = (await createRes.json()).data.id;

    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto('/notes');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    await page.getByText(noteTitle).first().click();

    await expect(page.getByRole('button', { name: 'Edit' })).toBeVisible({ timeout: 5000 });

    await page.getByRole('button', { name: 'Delete' }).click();
    await expect(page.getByRole('heading', { name: 'Delete note?' })).toBeVisible({ timeout: 5000 });

    await page.getByRole('button', { name: 'Delete' }).click();
    await expect(page.getByRole('heading', { name: 'Delete note?' })).toBeVisible({ timeout: 5000 });

    await page.getByRole('button', { name: 'Delete' }).click();

    const undoButton = page.getByRole('button', { name: 'Undo' });
    await expect(undoButton).toBeVisible({ timeout: 5000 });
    await undoButton.click();
    await page.waitForTimeout(1000);

    await page.reload();
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    await expect(page.getByText(noteTitle).first()).toBeVisible({ timeout: 5000 });

    const loginRes2 = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const c2 = loginRes2.headers()['set-cookie'] || '';
    const noteRes = await request.get(`http://localhost:3000/api/notes/${noteId}`, {
      headers: { Cookie: c2 },
    });
    expect(noteRes.status()).toBe(200);
  });

  test('link dropdown associates note with board', async ({ page, request }) => {
    const loginRes = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const cookies = loginRes.headers()['set-cookie'] || '';

    const ts = Date.now();
    const boardRes = await request.post('http://localhost:3000/api/boards', {
      data: { name: `LinkDropdownBoard ${ts}` },
      headers: { Cookie: cookies },
    });
    expect(boardRes.status()).toBe(201);
    const linkBoardId = (await boardRes.json()).data.id;

    const noteTitle = `Linked Note ${ts}`;
    const createRes = await request.post('http://localhost:3000/api/notes', {
      data: { title: noteTitle, content: 'Will be linked' },
      headers: { Cookie: cookies },
    });
    expect(createRes.status()).toBe(201);
    const noteId = (await createRes.json()).data.id;

    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL('/');

    await page.goto('/notes');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    await page.getByText(noteTitle).first().click();

    await expect(page.getByRole('button', { name: 'Edit' })).toBeVisible({ timeout: 5000 });

    await page.getByRole('button', { name: 'Edit' }).click();

    await expect(page.getByText('None')).toBeVisible({ timeout: 5000 });
    await page.getByText('None').click();

    const boardOption = page.locator('button').filter({ hasText: new RegExp(`LinkDropdownBoard ${ts}`) });
    await expect(boardOption).toBeVisible({ timeout: 5000 });
    await boardOption.click();
    await page.waitForTimeout(300);

    const editResponse = page.waitForResponse(
      (resp) => resp.url().includes('/api/notes/') && resp.request().method() === 'PATCH',
    );
    await page.getByRole('button', { name: 'Save' }).click();
    await editResponse;

    await page.waitForTimeout(500);
    await expect(page.getByText(noteTitle)).toBeVisible({ timeout: 5000 });

    await page.reload();
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    await expect(page.getByText(noteTitle).first()).toBeVisible({ timeout: 5000 });

    const loginRes2 = await request.post('http://localhost:3000/api/auth/login', {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    const c2 = loginRes2.headers()['set-cookie'] || '';
    const noteRes = await request.get(`http://localhost:3000/api/notes/${noteId}`, {
      headers: { Cookie: c2 },
    });
    expect(noteRes.status()).toBe(200);
    const noteData = await noteRes.json();
    expect(noteData.data.board_id).toBe(linkBoardId);
  });
});
