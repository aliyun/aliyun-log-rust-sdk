const { test } = require('node:test');
const assert = require('node:assert/strict');
const { platformStatus, badge, isOlder, publish } = require('./build_status.cjs');

const abi3 = 'wheel / manylinux2014-x86_64-abi3-3.12';
const native = 'wheel / manylinux2014-x86_64-native-3.12';
const job = (name, conclusion, id = 1) => ({ name, conclusion, id });

test('ABI3 badges are independent of native and other-platform failures', () => {
  assert.equal(platformStatus([abi3], [job(abi3, 'success'), job(native, 'failure'), job('other platform', 'failure')]), 'passing');
  assert.equal(platformStatus([abi3], [job(abi3, 'failure'), job(native, 'success')]), 'failing');
});

test('missing, skipped, cancelled and timed-out jobs never appear passing', () => {
  for (const conclusion of [null, 'skipped', 'neutral']) {
    assert.equal(platformStatus([abi3], [job(abi3, conclusion)]), 'incomplete');
  }
  assert.equal(platformStatus([abi3], []), 'incomplete');
  assert.equal(platformStatus([abi3], [job(abi3, 'cancelled')]), 'cancelled');
  assert.equal(platformStatus([abi3], [job(abi3, 'timed_out')]), 'failing');
});

test('newest attempt wins regardless of API ordering', () => {
  assert.equal(platformStatus([abi3], [job(abi3, 'failure', 20), job(abi3, 'success', 10)]), 'failing');
  assert.equal(platformStatus([abi3], [job(abi3, 'failure', 10), job(abi3, 'success', 20)]), 'passing');
});

test('publication ordering rejects old runs and duplicate attempts', () => {
  const previous = { run_id: 20, run_attempt: 2 };
  assert.equal(isOlder({ id: 19, run_attempt: 3 }, previous), true);
  assert.equal(isOlder({ id: 20, run_attempt: 2 }, previous), true);
  assert.equal(isOlder({ id: 20, run_attempt: 3 }, previous), false);
  assert.equal(isOlder({ id: 21, run_attempt: 1 }, previous), false);
  assert.match(badge('failing'), /<title>build: failing<\/title>/);
  assert.throws(() => badge('<script>'));
});

function fixture(previous) {
  const calls = {};
  const github = {
    rest: {
      git: {
        getRef: async () => {
          if (!previous) throw Object.assign(new Error('not found'), { status: 404 });
          return { data: { object: { sha: 'parent' } } };
        },
        createTree: async args => { calls.tree = args.tree; return { data: { sha: 'tree' } }; },
        createCommit: async args => { calls.commit = args; return { data: { sha: 'commit' } }; },
        createRef: async args => { calls.create = args; },
        updateRef: async args => { calls.update = args; },
      },
      repos: { getContent: async () => ({ data: { content: Buffer.from(JSON.stringify(previous)).toString('base64') } }) },
      actions: { listJobsForWorkflowRun: () => {} },
    },
    paginate: async (_method, args) => {
      calls.query = args;
      return [job(abi3, 'success'), job(native, 'failure')];
    },
  };
  const context = {
    repo: { owner: 'owner', repo: 'repo' },
    payload: { workflow_run: { id: 20, run_attempt: 2, head_sha: 'source', html_url: 'https://example.com/run/20' } },
  };
  return { calls, args: { github, context, core: { info: () => {} }, targets: { 'manylinux2014-x86_64': [abi3] } } };
}

test('first publication creates an isolated status branch with SVG and provenance', async () => {
  const { calls, args } = fixture();
  await publish(args);
  assert.equal(calls.create.ref, 'refs/heads/python-build-status');
  assert.deepEqual(calls.commit.parents, []);
  assert.equal(calls.query.filter, 'all');
  assert.equal(calls.query.per_page, 100);
  assert.equal(calls.tree.length, 2);
  assert.match(calls.tree[0].content, /build: passing/);
  const summary = JSON.parse(calls.tree[1].content);
  assert.equal(summary.commit, 'source');
  assert.equal(summary.run_attempt, 2);
});

test('subsequent publication fast-forwards only the dedicated branch', async () => {
  const { calls, args } = fixture({ run_id: 20, run_attempt: 1 });
  await publish(args);
  assert.deepEqual(calls.commit.parents, ['parent']);
  assert.equal(calls.update.ref, 'heads/python-build-status');
  assert.equal(calls.update.force, false);
  assert.equal(calls.create, undefined);
});

test('late completion of an older run performs no reads of jobs or writes', async () => {
  const { calls, args } = fixture({ run_id: 21, run_attempt: 1 });
  await publish(args);
  assert.deepEqual(calls, {});
});
