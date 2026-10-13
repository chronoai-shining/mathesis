import { unpackSource } from './sourceBundle';

export interface LeanProject {
  modules: Record<string, string>;
  project_files: Record<string, string>;
  solution: string;
  toolchain: string;
}
export async function readLeanProject(selected: File[]): Promise<LeanProject> {
  let files: Record<string, Uint8Array>;
  if (selected.length === 1 && selected[0]!.size > 30 * 1024 * 1024)
    throw new Error('Project archive must be at most 30 MB.');
  if (selected.length === 1 && /\.(zip|tgz|tar\.gz)$/i.test(selected[0]!.name))
    files = unpackSource(new Uint8Array(await selected[0]!.arrayBuffer()), selected[0]!.name);
  else {
    files = Object.create(null) as Record<string, Uint8Array>;
    let total = 0;
    for (const file of selected) {
      const path = (file.webkitRelativePath || file.name)
        .split('/')
        .slice(file.webkitRelativePath ? 1 : 0)
        .join('/');
      if (path.split('/').some((p) => p === '.git' || p === '.lake')) continue;
      if (
        !path.endsWith('.lean') &&
        !/^(lean-toolchain|lake-manifest.json|lakefile.toml|README.md|LICENSE(?:\.md|\.txt)?)$/.test(
          path,
        )
      )
        continue;
      total += file.size;
      if (total > 9 * 1048576)
        throw new Error('Project exceeds 8 MB Lean source plus 1 MB configuration.');
      files[path] = new Uint8Array(await file.arrayBuffer());
    }
  }
  const paths = Object.keys(files).filter(
    (path) => !path.split('/').some((p) => p === '.git' || p === '.lake'),
  );
  const roots = paths.filter((path) => path.endsWith('/lean-toolchain'));
  const root =
    !files['lean-toolchain'] && roots.length === 1
      ? roots[0]!.slice(0, -'lean-toolchain'.length)
      : '';
  const project: LeanProject = { modules: {}, project_files: {}, solution: '', toolchain: '' };
  let size = 0;
  for (const fullPath of paths) {
    if (root && !fullPath.startsWith(root)) continue;
    const path = fullPath.slice(root.length);
    const text = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(
      files[fullPath]!,
    );
    if (
      /^(lean-toolchain|lake-manifest.json|lakefile\.(lean|toml)|README.md|LICENSE(?:\.md|\.txt)?)$/.test(
        path,
      )
    )
      project.project_files[path] = text;
    else if (path === 'Solution.lean') project.solution = text;
    else if (path.endsWith('.lean')) {
      size += files[fullPath]!.length;
      const name = path.slice(0, -5).replaceAll('/', '.');
      if (Object.keys(project.modules).some((n) => n.toLowerCase() === name.toLowerCase()))
        throw new Error(`Duplicate Lean module: ${name}`);
      project.modules[name] = text;
    }
  }
  if (
    Object.keys(project.modules).length > 512 ||
    size + new TextEncoder().encode(project.solution).length > 8 * 1048576
  )
    throw new Error('Use at most 512 modules and 8 MB of Lean source.');
  if (!Object.keys(project.modules).length && !project.solution)
    throw new Error('No Lean source modules found.');
  project.toolchain = project.project_files['lean-toolchain']?.trim() ?? '';
  if (project.project_files['lake-manifest.json']) {
    const manifest = JSON.parse(project.project_files['lake-manifest.json']) as {
      packages?: { name: string; rev?: string }[];
    };
    const revision = manifest.packages?.find((p) => p.name === 'mathlib')?.rev;
    if (revision) project.toolchain += `, Mathlib ${revision}`;
  }
  return project;
}
