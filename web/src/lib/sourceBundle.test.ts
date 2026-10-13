import { describe, expect, it } from 'vitest';
import { strToU8, unzipSync, zipSync } from 'fflate';
import { readLeanProject } from './leanProject';
import { unpackSource, withEvidence } from './sourceBundle';

function file(name: string, data: string | Uint8Array): File {
  const bytes = typeof data === 'string' ? strToU8(data) : data;
  const result = new File([bytes as Uint8Array<ArrayBuffer>], name);
  Object.defineProperty(result, 'arrayBuffer', {
    value: async () => bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength),
  });
  return result;
}
describe('research source intake', () => {
  it('preserves a real-sized nested Lean project and pinned configuration', async () => {
    const files: Record<string, Uint8Array> = {
      'research/lean-toolchain': strToU8('leanprover/lean4:v4.33.0'),
      'research/lake-manifest.json': strToU8(
        JSON.stringify({ packages: [{ name: 'mathlib', rev: 'a'.repeat(40) }] }),
      ),
    };
    for (let i = 0; i < 80; i++)
      files[`research/Research/Part${i}.lean`] = strToU8(
        'namespace Research\ntheorem result : True := True.intro\nend Research',
      );
    const project = await readLeanProject([file('project.zip', zipSync(files))]);
    expect(Object.keys(project.modules)).toHaveLength(80);
    expect(project.modules['Research.Part79']).toContain('namespace Research');
    expect(project.toolchain).toBe(`leanprover/lean4:v4.33.0, Mathlib ${'a'.repeat(40)}`);
    expect(project.solution).toBe('');
  });
  it('binds certificates to the same paper archive without replacing the manuscript', async () => {
    const source = file('paper.tex', '\\title{A paper}');
    const result = await withEvidence(source, [
      {
        file: file('certificate.json', '{"cases":41}'),
        kind: 'certificate',
        description: 'Exhaustive cases for C1',
        claims: ['C1'],
        source_revision: 'b'.repeat(40),
      },
    ]);
    // jsdom File lacks arrayBuffer; read the created file through FileReader.
    const bytes = await new Promise<ArrayBuffer>((resolve) => {
      const reader = new FileReader();
      reader.onload = () => resolve(reader.result as ArrayBuffer);
      reader.readAsArrayBuffer(result);
    });
    const files = unzipSync(new Uint8Array(bytes));
    expect(new TextDecoder().decode(files['paper.tex'])).toBe('\\title{A paper}');
    const manifest = JSON.parse(new TextDecoder().decode(files['mathesis-evidence.json'])) as {
      files: { path: string; source_revision: string }[];
    };
    expect(manifest.files[0]).toMatchObject({
      path: 'evidence/certificate.json',
      source_revision: 'b'.repeat(40),
    });
  });
  it('rejects traversal and duplicate attachments instead of replacing data', async () => {
    expect(() => unpackSource(zipSync({ '../escape.tex': strToU8('bad') }), 'bad.zip')).toThrow(
      'Unsafe',
    );
    await expect(
      withEvidence(
        file('paper.tex', 'source'),
        [0, 1].map(() => ({
          file: file('same.txt', 'x'),
          kind: 'data' as const,
          description: 'data',
          claims: [],
        })),
      ),
    ).rejects.toThrow('already exists');
  });
});
