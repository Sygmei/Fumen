export interface LogTextPart {
    kind: "text" | "link";
    text: string;
    href?: string;
}

export interface ParsedLogLine {
    raw: string;
    timestamp?: string;
    level?: string;
    message: string;
    metadata?: string;
}

export type LogProviderParseResult =
    | {
          type: "groupStart";
          title: string;
          raw: string;
      }
    | {
          type: "groupEnd";
          raw: string;
      }
    | {
          type: "line";
          line: ParsedLogLine;
      }
    | {
          type: "skip";
      };

export interface LogProvider {
    parseLine(line: string, lineIndex: number): LogProviderParseResult;
    normalizeLevel?(level: string): string;
    linkify?(text: string, line: ParsedLogLine): LogTextPart[];
    levelOrder?: string[];
    displayLevel?(level: string): string;
}

export interface LogLineEntry {
    kind: "line";
    id: string;
    raw: string;
    sourceLine: number;
    timestamp?: string;
    level?: string;
    normalizedLevel?: string;
    message: string;
    metadata?: string;
    messageParts: LogTextPart[];
}

export interface LogGroupEntry {
    kind: "group";
    id: string;
    raw: string;
    sourceLine: number;
    title: string;
    entries: LogEntry[];
}

export type LogEntry = LogLineEntry | LogGroupEntry;

export interface LogBlockProps {
    logs?: string | string[];
    provider?: LogProvider;
    title?: string;
    emptyMessage?: string;
    showLevelFilter?: boolean;
    showDownloadButton?: boolean;
    downloadFileName?: string;
    initialGroupsCollapsed?: boolean;
    selectedLevels?: string[];
    className?: string;
}
