import { execFile } from 'child_process';
import { promisify } from 'util';

const execFileAsync = promisify(execFile);

export interface Scratch {
  id: string;
  project: string;
  title: string;
  created_at: string;
}

export interface PathResult {
  path: string;
}

export interface NukeResult {
  deleted_count: number;
  scope: string;
  project_name?: string;
}

export interface PadzOptions {
  cwd?: string;
}

export interface ListOptions {
  all?: boolean;
  global?: boolean;
}

export interface ViewOptions {
  all?: boolean;
  global?: boolean;
}

export interface SearchOptions {
  all?: boolean;
  global?: boolean;
}

export interface PeekOptions {
  all?: boolean;
  global?: boolean;
  lines?: number;
}

export class PadzClient {
  private cwd?: string;

  constructor(options?: PadzOptions) {
    this.cwd = options?.cwd;
  }

  private async exec(args: string[]): Promise<string> {
    try {
      const { stdout, stderr } = await execFileAsync('padz', args, {
        cwd: this.cwd,
        encoding: 'utf8',
      });

      // Check if stdout contains an error (JSON format)
      if (stdout) {
        try {
          const parsed = JSON.parse(stdout);
          if (parsed.error) {
            throw new Error(parsed.error);
          }
        } catch (e) {
          // Not JSON or no error field, continue
        }
      }

      return stdout;
    } catch (error: any) {
      // Handle command not found
      if (error.code === 'ENOENT') {
        throw new Error('padz command not found. Please ensure padz is installed and in PATH');
      }

      // Try to parse error from stderr/stdout
      const output = error.stdout || error.stderr || '';
      try {
        const parsed = JSON.parse(output);
        if (parsed.error) {
          throw new Error(parsed.error);
        }
      } catch (e) {
        // Not JSON, use original error
      }

      throw error;
    }
  }

  async create(content: string): Promise<Scratch> {
    // Use spawn for piping content
    const { spawn } = require('child_process');
    
    return new Promise((resolve, reject) => {
      const proc = spawn('padz', ['--format', 'json'], {
        cwd: this.cwd,
      });

      let stdout = '';
      let stderr = '';

      proc.stdout.on('data', (data: Buffer) => {
        stdout += data.toString();
      });

      proc.stderr.on('data', (data: Buffer) => {
        stderr += data.toString();
      });

      proc.on('close', (code: number) => {
        if (code !== 0) {
          try {
            const parsed = JSON.parse(stdout);
            if (parsed.error) {
              reject(new Error(parsed.error));
              return;
            }
          } catch (e) {
            // Not JSON
          }
          reject(new Error(stderr || `Process exited with code ${code}`));
          return;
        }

        try {
          const result = JSON.parse(stdout);
          resolve(result);
        } catch (e) {
          reject(new Error('Failed to parse JSON response'));
        }
      });

      proc.on('error', (error: Error) => {
        reject(error);
      });

      // Write content and close stdin
      proc.stdin.write(content);
      proc.stdin.end();
    });
  }

  async list(options?: ListOptions): Promise<Scratch[]> {
    const args = ['ls', '--format', 'json'];
    if (options?.all) args.push('--all');
    if (options?.global) args.push('--global');

    const stdout = await this.exec(args);
    return JSON.parse(stdout);
  }

  async view(index: number | string, options?: ViewOptions): Promise<string> {
    const args = ['view', index.toString(), '--format', 'json'];
    if (options?.all) args.push('--all');
    if (options?.global) args.push('--global');

    const stdout = await this.exec(args);
    const result = JSON.parse(stdout);
    return result.content;
  }

  async open(index: number | string, options?: { all?: boolean }): Promise<void> {
    const args = ['open', index.toString(), '--format', 'json'];
    if (options?.all) args.push('--all');

    await this.exec(args);
  }

  async peek(index: number | string, options?: PeekOptions): Promise<string> {
    const args = ['peek', index.toString(), '--format', 'json'];
    if (options?.all) args.push('--all');
    if (options?.global) args.push('--global');
    if (options?.lines !== undefined) {
      args.push('--lines', options.lines.toString());
    }

    const stdout = await this.exec(args);
    const result = JSON.parse(stdout);
    return result.content;
  }

  async delete(index: number | string, options?: { all?: boolean }): Promise<void> {
    const args = ['delete', index.toString(), '--format', 'json'];
    if (options?.all) args.push('--all');

    await this.exec(args);
  }

  async path(index: number | string, options?: { all?: boolean }): Promise<string> {
    const args = ['path', index.toString(), '--format', 'json'];
    if (options?.all) args.push('--all');

    const stdout = await this.exec(args);
    const result: PathResult = JSON.parse(stdout);
    return result.path;
  }

  async search(term: string, options?: SearchOptions): Promise<Scratch[]> {
    const args = ['search', term, '--format', 'json'];
    if (options?.all) args.push('--all');
    if (options?.global) args.push('--global');

    const stdout = await this.exec(args);
    return JSON.parse(stdout);
  }

  async cleanup(days?: number): Promise<void> {
    const args = ['cleanup', '--format', 'json'];
    if (days !== undefined) {
      args.push('--days', days.toString());
    }

    await this.exec(args);
  }

  async nuke(options?: { all?: boolean; yes?: boolean }): Promise<NukeResult> {
    const args = ['nuke', '--format', 'json'];
    if (options?.all) args.push('--all');
    if (options?.yes) args.push('--yes');

    const stdout = await this.exec(args);
    return JSON.parse(stdout);
  }

  // Helper method to create a new client with a different cwd
  withCwd(cwd: string): PadzClient {
    return new PadzClient({ cwd });
  }
}

// Export a default instance
export default new PadzClient();