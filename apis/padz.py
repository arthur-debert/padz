"""
padz.py - Python API for padz CLI

A simple wrapper around the padz CLI tool that provides a Pythonic interface
for managing scratches.
"""

import json
import subprocess
from dataclasses import dataclass
from datetime import datetime
from typing import List, Optional, Dict, Any


@dataclass
class Scratch:
    """Represents a padz scratch."""
    id: str
    project: str
    title: str
    created_at: datetime

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'Scratch':
        """Create a Scratch instance from a dictionary."""
        return cls(
            id=data['id'],
            project=data['project'],
            title=data['title'],
            created_at=datetime.fromisoformat(data['created_at'].replace('Z', '+00:00'))
        )


@dataclass
class PathResult:
    """Result from path command."""
    path: str


@dataclass
class NukeResult:
    """Result from nuke command."""
    deleted_count: int
    scope: str
    project_name: Optional[str] = None


class PadzError(Exception):
    """Exception raised by padz operations."""
    pass


class PadzClient:
    """Client for interacting with the padz CLI."""

    def __init__(self, cwd: Optional[str] = None):
        """
        Initialize a new PadzClient.

        Args:
            cwd: Working directory for padz commands. If None, uses current directory.
        """
        self.cwd = cwd

    def _exec(self, args: List[str], input_text: Optional[str] = None) -> str:
        """
        Execute a padz command and return the output.

        Args:
            args: Command arguments (without 'padz' prefix)
            input_text: Optional text to pipe to stdin

        Returns:
            stdout from the command

        Raises:
            PadzError: If the command fails or returns an error
        """
        cmd = ['padz'] + args

        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                cwd=self.cwd,
                input=input_text,
                check=False  # Don't raise on non-zero exit
            )

            # Check for errors in stdout (JSON format)
            if result.stdout:
                try:
                    data = json.loads(result.stdout)
                    if isinstance(data, dict) and 'error' in data:
                        raise PadzError(data['error'])
                except json.JSONDecodeError:
                    # Not JSON, continue
                    pass

            # Check exit code
            if result.returncode != 0:
                # Try to parse error from stderr or stdout
                error_msg = result.stderr or result.stdout or f"Command failed with exit code {result.returncode}"
                try:
                    error_data = json.loads(error_msg)
                    if isinstance(error_data, dict) and 'error' in error_data:
                        raise PadzError(error_data['error'])
                except json.JSONDecodeError:
                    pass
                raise PadzError(error_msg.strip())

            return result.stdout

        except FileNotFoundError:
            raise PadzError("padz command not found. Please ensure padz is installed and in PATH")
        except subprocess.SubprocessError as e:
            raise PadzError(f"Failed to execute command: {e}")

    def create(self, content: str) -> Scratch:
        """
        Create a new scratch with the given content.

        Args:
            content: The content of the scratch

        Returns:
            The created Scratch object

        Raises:
            PadzError: If creation fails
        """
        if not content:
            raise ValueError("Content cannot be empty")

        output = self._exec(['--format', 'json'], input_text=content)
        data = json.loads(output)
        return Scratch.from_dict(data)

    def list(self, all: bool = False, global_: bool = False) -> List[Scratch]:
        """
        List all scratches.

        Args:
            all: Include scratches from all projects
            global_: Include global scratches

        Returns:
            List of Scratch objects
        """
        args = ['ls', '--format', 'json']
        if all:
            args.append('--all')
        if global_:
            args.append('--global')

        output = self._exec(args)
        data = json.loads(output)
        return [Scratch.from_dict(item) for item in data]

    def view(self, index: int, all: bool = False, global_: bool = False) -> str:
        """
        View the content of a scratch.

        Args:
            index: The index of the scratch (1-based)
            all: Look in all projects
            global_: Look in global scratches

        Returns:
            The content of the scratch
        """
        args = ['view', str(index), '--format', 'json']
        if all:
            args.append('--all')
        if global_:
            args.append('--global')

        output = self._exec(args)
        data = json.loads(output)
        return data['content']

    def open(self, index: int, all: bool = False) -> None:
        """
        Open a scratch in the default editor.

        Args:
            index: The index of the scratch (1-based)
            all: Look in all projects
        """
        args = ['open', str(index), '--format', 'json']
        if all:
            args.append('--all')

        self._exec(args)

    def peek(self, index: int, all: bool = False, global_: bool = False, lines: int = 3) -> str:
        """
        Peek at the first/last lines of a scratch.

        Args:
            index: The index of the scratch (1-based)
            all: Look in all projects
            global_: Look in global scratches
            lines: Number of lines to show from start and end

        Returns:
            The preview content
        """
        args = ['peek', str(index), '--format', 'json', '--lines', str(lines)]
        if all:
            args.append('--all')
        if global_:
            args.append('--global')

        output = self._exec(args)
        data = json.loads(output)
        return data['content']

    def delete(self, index: int, all: bool = False) -> None:
        """
        Delete a scratch.

        Args:
            index: The index of the scratch (1-based)
            all: Look in all projects
        """
        args = ['delete', str(index), '--format', 'json']
        if all:
            args.append('--all')

        self._exec(args)

    def path(self, index: int, all: bool = False) -> str:
        """
        Get the file path of a scratch.

        Args:
            index: The index of the scratch (1-based)
            all: Look in all projects

        Returns:
            The absolute file path
        """
        args = ['path', str(index), '--format', 'json']
        if all:
            args.append('--all')

        output = self._exec(args)
        data = json.loads(output)
        return data['path']

    def search(self, term: str, all: bool = False, global_: bool = False) -> List[Scratch]:
        """
        Search for scratches containing the given term.

        Args:
            term: Search term (regex supported)
            all: Search in all projects
            global_: Search in global scratches

        Returns:
            List of matching Scratch objects
        """
        args = ['search', term, '--format', 'json']
        if all:
            args.append('--all')
        if global_:
            args.append('--global')

        output = self._exec(args)
        data = json.loads(output)
        return [Scratch.from_dict(item) for item in data]

    def cleanup(self, days: int = 30) -> None:
        """
        Clean up old scratches.

        Args:
            days: Delete scratches older than this many days
        """
        args = ['cleanup', '--format', 'json', '--days', str(days)]
        self._exec(args)

    def nuke(self, all: bool = False, yes: bool = False) -> NukeResult:
        """
        Delete all scratches in the current scope.

        Args:
            all: Delete from all projects
            yes: Skip confirmation (required for non-interactive use)

        Returns:
            NukeResult with deletion details

        Raises:
            PadzError: If yes is not True (interactive confirmation not supported)
        """
        if not yes:
            raise PadzError("Must set yes=True for non-interactive nuke operation")

        args = ['nuke', '--format', 'json', '--yes']
        if all:
            args.append('--all')

        output = self._exec(args)
        data = json.loads(output)
        return NukeResult(
            deleted_count=data['deleted_count'],
            scope=data['scope'],
            project_name=data.get('project_name')
        )

    def with_cwd(self, cwd: str) -> 'PadzClient':
        """
        Create a new client with a different working directory.

        Args:
            cwd: The working directory to use

        Returns:
            A new PadzClient instance
        """
        return PadzClient(cwd=cwd)


# Convenience functions using default client
_default_client = PadzClient()


def create(content: str) -> Scratch:
    """Create a new scratch with the given content."""
    return _default_client.create(content)


def list(all: bool = False, global_: bool = False) -> List[Scratch]:
    """List all scratches."""
    return _default_client.list(all=all, global_=global_)


def view(index: int, all: bool = False, global_: bool = False) -> str:
    """View the content of a scratch."""
    return _default_client.view(index, all=all, global_=global_)


def open(index: int, all: bool = False) -> None:
    """Open a scratch in the default editor."""
    return _default_client.open(index, all=all)


def peek(index: int, all: bool = False, global_: bool = False, lines: int = 3) -> str:
    """Peek at the first/last lines of a scratch."""
    return _default_client.peek(index, all=all, global_=global_, lines=lines)


def delete(index: int, all: bool = False) -> None:
    """Delete a scratch."""
    return _default_client.delete(index, all=all)


def path(index: int, all: bool = False) -> str:
    """Get the file path of a scratch."""
    return _default_client.path(index, all=all)


def search(term: str, all: bool = False, global_: bool = False) -> List[Scratch]:
    """Search for scratches containing the given term."""
    return _default_client.search(term, all=all, global_=global_)


def cleanup(days: int = 30) -> None:
    """Clean up old scratches."""
    return _default_client.cleanup(days=days)


def nuke(all: bool = False, yes: bool = False) -> NukeResult:
    """Delete all scratches in the current scope."""
    return _default_client.nuke(all=all, yes=yes)