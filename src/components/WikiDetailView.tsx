import { useState, useRef, useMemo } from "react";
import type { ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { Components } from "react-markdown";
import {
  Box,
  Button,
  Divider,
  Flex,
  Group,
  ScrollArea,
  Stack,
  Tabs,
  Text,
  Textarea,
  Title,
} from "@mantine/core";

// ── Markdown custom components ───────────────────────────────────

const mdComponents: Components = {
  h1: ({ children, ...props }) => {
    const id = "heading-" + String(children).replace(/\s+/g, "-");
    return (
      <h1
        data-heading-id={id}
        style={{
          fontSize: "1.3rem",
          fontWeight: 700,
          margin: "2rem 0 0.75rem",
          color: "var(--color-label-primary)",
          letterSpacing: "-0.02em",
        }}
        {...props}
      >
        {children}
      </h1>
    );
  },
  h2: ({ children, ...props }) => {
    const id = "heading-" + String(children).replace(/\s+/g, "-");
    return (
      <h2
        data-heading-id={id}
        style={{
          fontSize: "1.1rem",
          fontWeight: 600,
          margin: "2rem 0 0.6rem",
          paddingBottom: "0.35rem",
          borderBottom: "1px solid var(--color-separator)",
          color: "var(--color-label-primary)",
        }}
        {...props}
      >
        {children}
      </h2>
    );
  },
  h3: ({ children, ...props }) => {
    const id = "heading-" + String(children).replace(/\s+/g, "-");
    return (
      <h3
        data-heading-id={id}
        style={{
          fontSize: "0.95rem",
          fontWeight: 600,
          margin: "1.5rem 0 0.5rem",
          color: "var(--color-label-primary)",
        }}
        {...props}
      >
        {children}
      </h3>
    );
  },
  p: ({ children, ...props }) => (
    <p
      style={{
        margin: "0.6rem 0",
        lineHeight: 1.85,
        color: "var(--color-label-secondary)",
        fontSize: "0.82rem",
      }}
      {...props}
    >
      {children}
    </p>
  ),
  ul: ({ children, ...props }) => (
    <ul style={{ margin: "0.5rem 0", paddingLeft: "1.5rem", lineHeight: 1.85 }} {...props}>
      {children}
    </ul>
  ),
  ol: ({ children, ...props }) => (
    <ol style={{ margin: "0.5rem 0", paddingLeft: "1.5rem", lineHeight: 1.85 }} {...props}>
      {children}
    </ol>
  ),
  li: ({ children, ...props }) => (
    <li
      style={{
        margin: "0.3rem 0",
        fontSize: "0.82rem",
        color: "var(--color-label-secondary)",
      }}
      {...props}
    >
      {children}
    </li>
  ),
  strong: ({ children, ...props }) => (
    <strong style={{ color: "var(--color-label-primary)", fontWeight: 600 }} {...props}>
      {children}
    </strong>
  ),
  blockquote: ({ children, ...props }) => (
    <blockquote
      style={{
        margin: "0.75rem 0",
        paddingLeft: "1rem",
        borderLeft: "3px solid var(--color-accent)",
        color: "var(--color-label-tertiary)",
        fontStyle: "italic",
      }}
      {...props}
    >
      {children}
    </blockquote>
  ),
  hr: (props) => (
    <hr
      style={{
        border: "none",
        borderTop: "1px solid var(--color-separator)",
        margin: "1.5rem 0",
      }}
      {...props}
    />
  ),
  a: ({ children, href, ...props }) => (
    <a href={href} style={{ color: "var(--color-accent)", textDecoration: "none" }} {...props}>
      {children}
    </a>
  ),
};

// ── Outline ──────────────────────────────────────────────────────

function Outline({
  content,
  containerRef,
}: {
  content: string;
  containerRef: React.RefObject<HTMLDivElement | null>;
}) {
  const headings = useMemo(() => {
    const result: { level: number; text: string; id: string }[] = [];
    for (const line of content.split("\n")) {
      const match = line.match(/^(#{1,4})\s+(.+)/);
      if (match) {
        const text = match[2].trim();
        result.push({
          level: match[1].length,
          text,
          id: "heading-" + text.replace(/\s+/g, "-"),
        });
      }
    }
    return result;
  }, [content]);

  if (headings.length === 0) return null;

  const handleClick = (id: string) => {
    const el = containerRef.current?.querySelector(`[data-heading-id="${id}"]`);
    if (el) el.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  return (
    <Stack gap={2}>
      <Text
        size="xs"
        tt="uppercase"
        c="var(--color-label-quaternary)"
        fw={600}
        mb="0.5rem"
        style={{ letterSpacing: "0.06em", fontSize: "0.68rem" }}
      >
        目录
      </Text>
      {headings.map((h, i) => (
        <Text
          key={i}
          size="xs"
          c="var(--color-label-secondary)"
          truncate
          onClick={() => handleClick(h.id)}
          style={{
            paddingLeft: `${(h.level - 1) * 12}px`,
            cursor: "pointer",
            lineHeight: 2,
          }}
        >
          {h.text}
        </Text>
      ))}
    </Stack>
  );
}

// ── Wiki Preview ─────────────────────────────────────────────────

function WikiPreview({ content }: { content: string }) {
  if (!content) {
    return (
      <Text c="var(--color-label-quaternary)" fs="italic" size="sm">
        （暂无内容）
      </Text>
    );
  }
  return (
    <Box px="1rem">
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={mdComponents}>
        {content}
      </ReactMarkdown>
    </Box>
  );
}

// ── WikiDetailView (shared layout) ──────────────────────────────

export interface WikiDetailViewProps {
  title: ReactNode;
  subtitle?: string;
  description?: string;
  wikiContent: string;
  onWikiChange: (v: string) => void;
  onWikiSave: () => void;
  onDelete: () => void;
  deleteLabel?: string;
  footer?: ReactNode;
}

export function WikiDetailView({
  title,
  subtitle,
  description,
  wikiContent,
  onWikiChange,
  onWikiSave,
  onDelete,
  deleteLabel = "删除",
  footer,
}: WikiDetailViewProps) {
  const [mode, setMode] = useState<"preview" | "edit">("preview");
  const contentRef = useRef<HTMLDivElement>(null);

  return (
    <Flex direction="column" h="100%">
      {/* Header */}
      <Box ta="center" pt="1.5rem" pb="1rem" style={{ borderBottom: "1px solid var(--color-separator)" }}>
        <Title order={2} fz="1.5rem" fw={700} style={{ letterSpacing: "-0.03em" }}>
          {title}
        </Title>
        {subtitle && (
          <Text size="sm" c="var(--color-label-tertiary)" mt="0.3rem">
            {subtitle}
          </Text>
        )}
        {description && (
          <Text
            size="sm"
            c="var(--color-label-secondary)"
            mt="0.4rem"
            mx="auto"
            maw={500}
          >
            {description}
          </Text>
        )}
      </Box>

      {/* Body: content + outline */}
      <Flex ref={contentRef} style={{ flex: 1, overflow: "hidden" }}>
        <ScrollArea style={{ flex: 1 }}>
          <Box py="2rem">
            <Box w="60%" mx="auto">
              <Group justify="flex-end" gap="0.25rem" mb="1rem">
                <Tabs value={mode} onChange={(v) => v && setMode(v as "preview" | "edit")} variant="default">
                  <Tabs.List>
                    <Tabs.Tab value="preview">预览</Tabs.Tab>
                    <Tabs.Tab value="edit">编辑</Tabs.Tab>
                  </Tabs.List>
                </Tabs>
                <Button
                  variant="subtle"
                  size="compact-xs"
                  color="gray"
                  onClick={onDelete}
                  ml="auto"
                >
                  {deleteLabel}
                </Button>
              </Group>

              {mode === "edit" ? (
                <Textarea
                  value={wikiContent}
                  onChange={(e) => onWikiChange(e.currentTarget.value)}
                  onBlur={onWikiSave}
                  placeholder="支持 Markdown 格式..."
                  autosize
                  minRows={20}
                  maxRows={40}
                  styles={{
                    input: {
                      fontFamily: "monospace",
                      fontSize: "0.8rem",
                      lineHeight: 1.65,
                    },
                  }}
                />
              ) : (
                <WikiPreview content={wikiContent} />
              )}
            </Box>
          </Box>
        </ScrollArea>

        {wikiContent && (
          <ScrollArea
            w={180}
            style={{
              flexShrink: 0,
              borderLeft: "1px solid var(--color-separator)",
            }}
          >
            <Box px="0.75rem" py="1.25rem">
              {mode === "preview" && <Outline content={wikiContent} containerRef={contentRef} />}
            </Box>
          </ScrollArea>
        )}
      </Flex>

      {footer && (
        <>
          <Divider />
          <Box px="1.5rem" py="0.75rem">
            {footer}
          </Box>
        </>
      )}
    </Flex>
  );
}
