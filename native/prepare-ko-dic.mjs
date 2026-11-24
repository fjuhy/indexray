// Prepare the external Korean dictionary for the Windows release.

import { spawnSync } from 'node:child_process';
import { appendFileSync, cpSync, existsSync, readdirSync, rmSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';

const repoDir = resolve(import.meta.dirname, '..');
const destDir = join(repoDir, 'target', 'ko-dic');
const args = new Set(process.argv.slice(2));

function isDict(dir) {
	return Boolean(dir) && existsSync(join(dir, 'metadata.json'));
}

function listDirs(dir) {
	try {
		return readdirSync(dir, { withFileTypes: true })
			.filter((e) => e.isDirectory())
			.map((e) => join(dir, e.name));
	} catch {
		return [];
	}
}

/** 후보 중 metadata.json이 가장 최근인 사전 디렉터리. */
function newest(dirs) {
	return dirs
		.filter(isDict)
		.map((dir) => ({ dir, mtime: statSync(join(dir, 'metadata.json')).mtimeMs }))
		.sort((a, b) => b.mtime - a.mtime)[0]?.dir;
}

function findDictionary() {
	const env = process.env.DOWSE_KO_DIC;
	if (isDict(env)) return env;

	const cache = process.env.LINDERA_BUILD_DICTIONARY_CACHE_DIR;
	if (cache) {
		const hit = newest(listDirs(resolve(repoDir, cache)).map((d) => join(d, 'lindera-ko-dic')));
		if (hit) return hit;
	}

	const targetDir = process.env.CARGO_TARGET_DIR
		? resolve(repoDir, process.env.CARGO_TARGET_DIR)
		: join(repoDir, 'target');
	// target/<profile>/build 와 target/<triple>/<profile>/build 둘 다 본다. build 아래는
	// 예전 배치(lindera-ko-dic-<hash>/out)와 새 배치(lindera-ko-dic/<hash>/out) 둘 다 있다.
	const profileDirs = listDirs(targetDir).flatMap((d) => [d, ...listDirs(d)]);
	const buildOuts = profileDirs
		.flatMap((d) => listDirs(join(d, 'build')))
		.flatMap((d) => {
			if (/[\\/]lindera-ko-dic-[^\\/]+$/.test(d)) return [d];
			if (/[\\/]lindera-ko-dic$/.test(d)) return listDirs(d);
			return [];
		})
		.map((d) => join(d, 'out', 'lindera-ko-dic'));
	return newest(buildOuts);
}

/** lindera-ko-dic 크레이트의 NOTICE(mecab-ko-dic 라이선스 고지) 경로. */
function findNotice() {
	const res = spawnSync('cargo', ['metadata', '--format-version', '1', '--locked'], {
		cwd: repoDir,
		encoding: 'utf8',
		maxBuffer: 256 * 1024 * 1024
	});
	if (res.status !== 0) return undefined;
	const pkg = JSON.parse(res.stdout).packages.find((p) => p.name === 'lindera-ko-dic');
	if (!pkg) return undefined;
	const notice = join(dirname(pkg.manifest_path), 'NOTICE.txt');
	return existsSync(notice) ? notice : undefined;
}

function setOutput(ready) {
	if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `ready=${ready}\n`);
}

let source = findDictionary();
if (!source && args.has('--build')) {
	console.log('Building Korean dictionary with the IndexRay test dependencies…');
	const res = spawnSync('cargo', ['check', '--tests', '--locked'], {
		cwd: repoDir,
		stdio: 'inherit'
	});
	if (res.status === 0) source = findDictionary();
}

if (!source) {
	const msg =
		'ko-dic 사전을 찾지 못해 설치 파일에 넣지 않습니다(한글은 어절 단위로 색인됨). ' +
		'--build로 다시 실행하거나 DOWSE_KO_DIC을 지정하세요.';
	if (process.env.GITHUB_ACTIONS) console.log(`::warning::${msg}`);
	else console.warn(msg);
	setOutput(false);
	process.exit(args.has('--require') ? 1 : 0);
}

rmSync(destDir, { recursive: true, force: true });
cpSync(source, destDir, { recursive: true });
const notice = findNotice();
if (notice) cpSync(notice, join(destDir, 'NOTICE.txt'));

const files = readdirSync(destDir);
const bytes = files.reduce((sum, f) => sum + statSync(join(destDir, f)).size, 0);
console.log(
	`ko-dic 준비 완료: ${source} → ${destDir} (${files.length}개 파일, ${(bytes / 1024 / 1024).toFixed(1)}MB` +
		`${notice ? ', NOTICE 포함' : ', NOTICE 없음'})`
);
setOutput(true);
