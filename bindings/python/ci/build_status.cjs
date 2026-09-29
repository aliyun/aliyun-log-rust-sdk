// Executed by the trusted workflow_run observer, independently of build success.
const BRANCH = 'python-build-status';
const COLORS = {
  passing: '#4c1', failing: '#e05d44', cancelled: '#9f9f9f', incomplete: '#dfb317',
};

function platformStatus(expected, jobs) {
  // A partial rerun can include earlier successful jobs. Keep the newest result
  // for each name, and never let an old success hide a newer failed attempt.
  const latest = new Map();
  for (const job of jobs) {
    if (!latest.has(job.name) || latest.get(job.name).id < job.id) {
      latest.set(job.name, job);
    }
  }
  const conclusions = expected.map(name => latest.get(name)?.conclusion);
  if (conclusions.some(value => ['failure', 'timed_out', 'action_required', 'startup_failure', 'stale'].includes(value))) {
    return 'failing';
  }
  if (conclusions.includes('cancelled')) return 'cancelled';
  if (conclusions.length && conclusions.every(value => value === 'success')) return 'passing';
  return 'incomplete';
}

function badge(status) {
  // Only fixed status strings enter the SVG; no job names or branch text.
  if (!Object.hasOwn(COLORS, status)) throw new Error(`Unknown status: ${status}`);
  return `<svg xmlns="http://www.w3.org/2000/svg" width="144" height="20" role="img" aria-label="build: ${status}">
  <title>build: ${status}</title>
  <rect width="144" height="20" rx="3" fill="#555"/>
  <path d="M47 0h94q3 0 3 3v14q0 3-3 3H47z" fill="${COLORS[status]}"/>
  <g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" font-size="11">
    <text x="24" y="14">build</text><text x="95" y="14">${status}</text>
  </g>
</svg>\n`;
}

function isOlder(run, previous) {
  return run.id < previous.run_id ||
    (run.id === previous.run_id && run.run_attempt <= previous.run_attempt);
}

async function publish({ github, context, core, targets }) {
  const repo = context.repo;
  const run = context.payload.workflow_run;
  let parent;
  try {
    const ref = await github.rest.git.getRef({ ...repo, ref: `heads/${BRANCH}` });
    parent = ref.data.object.sha;
  } catch (error) {
    if (error.status !== 404) throw error;
  }
  if (parent) {
    const previousFile = await github.rest.repos.getContent({ ...repo, ref: parent, path: 'status.json' });
    const previous = JSON.parse(Buffer.from(previousFile.data.content, 'base64').toString('utf8'));
    if (isOlder(run, previous)) {
      core.info('A newer run or this attempt is already published.');
      return;
    }
  }

  // Paginate because job counts can grow with platforms and rerun attempts.
  // Include all attempts so rerunning only failures retains successful jobs;
  // platformStatus selects the newest execution of each job by its ID.
  const jobs = await github.paginate(github.rest.actions.listJobsForWorkflowRun, {
    ...repo, run_id: run.id, filter: 'all', per_page: 100,
  });
  const platforms = {};
  const tree = [];
  for (const [platform, expected] of Object.entries(targets)) {
    if (!/^[a-zA-Z0-9_-]+$/.test(platform) || !Array.isArray(expected) || !expected.length) {
      throw new Error(`Invalid platform definition: ${platform}`);
    }
    const status = platformStatus(expected, jobs);
    platforms[platform] = { status, expected_jobs: expected };
    tree.push({ path: `${platform}.svg`, mode: '100644', type: 'blob', content: badge(status) });
  }
  const summary = {
    run_id: run.id, run_attempt: run.run_attempt, url: run.html_url,
    commit: run.head_sha, platforms,
  };
  tree.push({ path: 'status.json', mode: '100644', type: 'blob', content: JSON.stringify(summary, null, 2) + '\n' });
  const createdTree = await github.rest.git.createTree({ ...repo, tree });
  const commit = await github.rest.git.createCommit({
    ...repo, message: `Python build status: run ${run.id}, attempt ${run.run_attempt}`,
    tree: createdTree.data.sha, parents: parent ? [parent] : [],
  });
  if (parent) {
    await github.rest.git.updateRef({ ...repo, ref: `heads/${BRANCH}`, sha: commit.data.sha, force: false });
  } else {
    await github.rest.git.createRef({ ...repo, ref: `refs/heads/${BRANCH}`, sha: commit.data.sha });
  }
  core.info(`Published ${Object.keys(platforms).length} platform badges for ${run.html_url}`);
}

module.exports = { platformStatus, badge, isOlder, publish };
