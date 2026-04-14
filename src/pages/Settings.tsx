import { useState, useEffect } from "react";
import type { ReactNode } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  ActionIcon,
  Box,
  Button,
  Checkbox,
  Group,
  NumberInput,
  PasswordInput,
  ScrollArea,
  Select,
  Stack,
  Text,
  TextInput,
  Textarea,
  Title,
} from "@mantine/core";
import {
  config,
  dataIO,
  tracker,
  type AppConfig,
  type MusicTrack,
  type TrackerStatus,
} from "../lib/tauri";

const LANG_OPTIONS = [
  { value: "zh", label: "中文 (zh)" },
  { value: "en", label: "English (en)" },
  { value: "ja", label: "日本語 (ja)" },
];

export default function Settings() {
  const [cfg, setCfg] = useState<AppConfig | null>(null);
  const [cachePath, setCachePath] = useState("");
  const [trackerStatus, setTrackerStatus] = useState<TrackerStatus | null>(null);
  const [trackerInput, setTrackerInput] = useState("");
  const [refreshing, setRefreshing] = useState(false);

  useEffect(() => {
    config.get().then(setCfg);
    config.getCachePath().then(setCachePath);
    tracker.getStatus().then(setTrackerStatus);
  }, []);

  const update = (mutate: (draft: AppConfig) => void) => {
    setCfg((prev) => {
      if (!prev) return null;
      const next: AppConfig = JSON.parse(JSON.stringify(prev));
      mutate(next);
      config.save(next).catch(console.error);
      return next;
    });
  };

  const pickDir = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") {
      update((c) => {
        c.library.root_dir = dir;
      });
    }
  };

  if (!cfg) {
    return (
      <Box p="2rem">
        <Text size="sm" c="var(--color-label-tertiary)">
          加载中...
        </Text>
      </Box>
    );
  }

  const updatePlaylist = (newPlaylist: MusicTrack[]) => {
    update((c) => {
      c.music.playlist = newPlaylist;
    });
  };

  return (
    <ScrollArea style={{ flex: 1 }}>
      <Box px="1.75rem" pt="1.4rem" pb="3rem">
        <Title order={1} mb="2rem" fz="1.6rem" fw={700} style={{ letterSpacing: "-0.035em" }}>
          设置
        </Title>

        <Section title="TMDB">
          <Field label="API Key">
            <PasswordInput
              defaultValue={cfg.tmdb.api_key}
              placeholder="在 themoviedb.org 免费申请"
              onBlur={(e) => {
                const v = e.currentTarget.value;
                update((c) => {
                  c.tmdb.api_key = v;
                });
              }}
            />
          </Field>
        </Section>

        <Section title="OpenSubtitles">
          <Field label="API Key">
            <PasswordInput
              defaultValue={cfg.opensubtitles.api_key}
              placeholder="必填"
              onBlur={(e) => {
                const v = e.currentTarget.value;
                update((c) => {
                  c.opensubtitles.api_key = v;
                });
              }}
            />
          </Field>
          <Field label="用户名">
            <TextInput
              defaultValue={cfg.opensubtitles.username}
              placeholder="可选，填写后下载配额更高"
              onBlur={(e) => {
                const v = e.currentTarget.value;
                update((c) => {
                  c.opensubtitles.username = v;
                });
              }}
            />
          </Field>
          <Field label="密码">
            <PasswordInput
              defaultValue={cfg.opensubtitles.password}
              placeholder="可选"
              onBlur={(e) => {
                const v = e.currentTarget.value;
                update((c) => {
                  c.opensubtitles.password = v;
                });
              }}
            />
          </Field>
        </Section>

        <Section title="ASSRT（射手网）">
          <Field label="Token">
            <PasswordInput
              defaultValue={cfg.assrt?.token ?? ""}
              placeholder="从 assrt.net 获取"
              onBlur={(e) => {
                const v = e.currentTarget.value;
                update((c) => {
                  c.assrt = { token: v };
                });
              }}
            />
          </Field>
        </Section>

        <Section title="字幕">
          <Field label="默认语言">
            <Select
              data={LANG_OPTIONS}
              value={cfg.subtitle.default_lang}
              onChange={(v) => {
                if (!v) return;
                update((c) => {
                  c.subtitle.default_lang = v;
                });
              }}
              w={180}
            />
          </Field>
        </Section>

        <Section title="工具路径">
          <Field label="ffmpeg">
            <TextInput
              defaultValue={cfg.tools.ffmpeg}
              placeholder="ffmpeg"
              onBlur={(e) => {
                const v = e.currentTarget.value;
                update((c) => {
                  c.tools.ffmpeg = v;
                });
              }}
            />
          </Field>
        </Section>

        <Section title="下载">
          <Field label="最大并发数">
            <NumberInput
              w={100}
              min={1}
              max={10}
              defaultValue={cfg.download?.max_concurrent ?? 3}
              onBlur={(e) => {
                const v = parseInt(e.currentTarget.value, 10);
                if (!isNaN(v) && v >= 1 && v <= 10) {
                  update((c) => {
                    c.download.max_concurrent = v;
                  });
                }
              }}
            />
          </Field>
          <Field label="启用 DHT">
            <Checkbox
              checked={cfg.download?.enable_dht ?? true}
              onChange={(e) => {
                const checked = e.currentTarget.checked;
                update((c) => {
                  c.download.enable_dht = checked;
                });
              }}
            />
          </Field>
          <Field label="会话持久化">
            <Checkbox
              checked={cfg.download?.persist_session ?? false}
              onChange={(e) => {
                const checked = e.currentTarget.checked;
                update((c) => {
                  c.download.persist_session = checked;
                });
              }}
            />
          </Field>
        </Section>

        <Section title="TRACKER 服务器">
          <Field label="自动维护">
            <Group gap="0.75rem">
              <Text size="sm" c="var(--color-label-tertiary)">
                {trackerStatus
                  ? `${trackerStatus.auto_count} 个服务器` +
                    (trackerStatus.last_updated
                      ? `，最后更新于 ${new Date(trackerStatus.last_updated).toLocaleString("zh-CN")}`
                      : "，从未更新")
                  : "加载中..."}
              </Text>
              <Button
                variant="default"
                size="xs"
                disabled={refreshing}
                loading={refreshing}
                onClick={async () => {
                  setRefreshing(true);
                  try {
                    const s = await tracker.refresh();
                    setTrackerStatus(s);
                  } catch (e) {
                    alert("更新失败: " + e);
                  } finally {
                    setRefreshing(false);
                  }
                }}
              >
                立即更新
              </Button>
            </Group>
          </Field>
          <Field label="自定义服务器">
            <Stack gap="0.5rem">
              <Textarea
                value={trackerInput}
                onChange={(e) => setTrackerInput(e.currentTarget.value)}
                placeholder={"每行一个 tracker 地址\nudp://tracker.example.com:1337/announce\nhttps://tracker.example.com/announce"}
                minRows={4}
                autosize
              />
              <Box>
                <Button
                  variant="default"
                  size="xs"
                  onClick={async () => {
                    if (!trackerInput.trim()) return;
                    try {
                      const s = await tracker.addUserTrackers(trackerInput);
                      setTrackerStatus(s);
                      setTrackerInput("");
                    } catch (e) {
                      alert("添加失败: " + e);
                    }
                  }}
                >
                  添加
                </Button>
              </Box>
            </Stack>
          </Field>
        </Section>

        <Section title="库目录">
          <Field label="本地库根目录">
            <Group gap="0.5rem">
              <TextInput value={cfg.library.root_dir} readOnly style={{ flex: 1 }} />
              <Button variant="default" size="xs" onClick={pickDir}>
                选择...
              </Button>
            </Group>
          </Field>
        </Section>

        <Section title="搜索">
          <Field label="请求间隔（秒）">
            <NumberInput
              w={100}
              min={0}
              defaultValue={cfg.search.rate_limit_secs}
              onBlur={(e) => {
                const v = parseInt(e.currentTarget.value, 10);
                if (!isNaN(v) && v >= 0) {
                  update((c) => {
                    c.search.rate_limit_secs = v;
                  });
                }
              }}
            />
          </Field>
        </Section>

        <Section title="缓存">
          <Field label="缓存文件路径">
            <TextInput value={cachePath} readOnly style={{ flex: 1 }} />
          </Field>
          <Field label="最大缓存条目">
            <NumberInput
              w={120}
              min={1}
              defaultValue={cfg.cache?.max_entries ?? 200}
              onBlur={(e) => {
                const v = parseInt(e.currentTarget.value, 10);
                if (!isNaN(v) && v > 0) {
                  update((c) => {
                    c.cache.max_entries = v;
                  });
                }
              }}
            />
          </Field>
        </Section>

        <Section title="背景音乐">
          <Field label="启用">
            <Checkbox
              checked={!!cfg.music?.enabled}
              onChange={(e) => {
                const checked = e.currentTarget.checked;
                update((c) => {
                  c.music.enabled = checked;
                });
              }}
            />
          </Field>
          <Field label="播放模式">
            <Select
              data={[
                { value: "sequential", label: "顺序播放" },
                { value: "random", label: "随机播放" },
              ]}
              value={cfg.music?.mode ?? "sequential"}
              onChange={(v) => {
                if (!v) return;
                const mode = v === "random" ? ("random" as const) : ("sequential" as const);
                update((c) => {
                  c.music.mode = mode;
                });
              }}
              w={180}
            />
          </Field>
          <Field label="播放列表">
            <Stack gap="0.4rem">
              {(cfg.music?.playlist ?? []).map((track, i) => (
                <Group key={i} gap="0.5rem" wrap="nowrap">
                  <TextInput
                    placeholder="曲目名称"
                    defaultValue={track.name}
                    w={140}
                    onBlur={(e) => {
                      const v = e.currentTarget.value;
                      const pl = [...(cfg.music?.playlist ?? [])];
                      pl[i] = { ...pl[i], name: v };
                      updatePlaylist(pl);
                    }}
                  />
                  <TextInput
                    placeholder="文件路径或 URL"
                    defaultValue={track.src}
                    style={{ flex: 1 }}
                    onBlur={(e) => {
                      const v = e.currentTarget.value;
                      const pl = [...(cfg.music?.playlist ?? [])];
                      pl[i] = { ...pl[i], src: v };
                      updatePlaylist(pl);
                    }}
                  />
                  <ActionIcon
                    variant="subtle"
                    color="gray"
                    onClick={() => {
                      const pl = (cfg.music?.playlist ?? []).filter((_, idx) => idx !== i);
                      updatePlaylist(pl);
                    }}
                  >
                    ✕
                  </ActionIcon>
                </Group>
              ))}
              <Button
                variant="default"
                size="xs"
                style={{ borderStyle: "dashed", alignSelf: "flex-start" }}
                onClick={() => {
                  const pl = [...(cfg.music?.playlist ?? []), { name: "", src: "" }];
                  updatePlaylist(pl);
                }}
              >
                + 添加曲目
              </Button>
            </Stack>
          </Field>
        </Section>

        <Section title="云同步">
          <Field label="Endpoint">
            <TextInput
              defaultValue={cfg.sync?.endpoint ?? ""}
              placeholder="http://192.168.1.x:3900"
              onBlur={(e) => {
                const v = e.currentTarget.value;
                update((c) => {
                  c.sync.endpoint = v;
                });
              }}
            />
          </Field>
          <Field label="Bucket">
            <TextInput
              defaultValue={cfg.sync?.bucket ?? ""}
              placeholder="blowup"
              onBlur={(e) => {
                const v = e.currentTarget.value;
                update((c) => {
                  c.sync.bucket = v;
                });
              }}
            />
          </Field>
          <Field label="Access Key">
            <PasswordInput
              defaultValue={cfg.sync?.access_key ?? ""}
              onBlur={(e) => {
                const v = e.currentTarget.value;
                update((c) => {
                  c.sync.access_key = v;
                });
              }}
            />
          </Field>
          <Field label="Secret Key">
            <PasswordInput
              defaultValue={cfg.sync?.secret_key ?? ""}
              onBlur={(e) => {
                const v = e.currentTarget.value;
                update((c) => {
                  c.sync.secret_key = v;
                });
              }}
            />
          </Field>
          <Field label="连接测试">
            <Button
              variant="default"
              size="xs"
              onClick={async () => {
                try {
                  const msg = await dataIO.testS3Connection();
                  alert(msg);
                } catch (e) {
                  alert("连接失败: " + e);
                }
              }}
            >
              测试连接
            </Button>
          </Field>
        </Section>

        <Section title="数据管理">
          <DataIORow
            label="知识库"
            onLocal={async (dir) => {
              if (dir === "export") {
                const path = await save({
                  defaultPath: "blowup-knowledge-base.json",
                  filters: [{ name: "JSON", extensions: ["json"] }],
                });
                if (path) {
                  await dataIO.exportKnowledgeBase(path);
                  alert("知识库导出成功");
                }
              } else {
                const path = await open({ filters: [{ name: "JSON", extensions: ["json"] }] });
                if (path) {
                  const msg = await dataIO.importKnowledgeBase(path as string);
                  alert(msg);
                  config.get().then(setCfg);
                }
              }
            }}
            onCloud={async (dir) => {
              if (dir === "export") {
                await dataIO.exportKnowledgeBaseS3();
                alert("知识库已导出到云端");
              } else {
                const msg = await dataIO.importKnowledgeBaseS3();
                alert(msg);
                config.get().then(setCfg);
              }
            }}
          />
          <DataIORow
            label="配置文件"
            onLocal={async (dir) => {
              if (dir === "export") {
                const path = await save({
                  defaultPath: "blowup-config.toml",
                  filters: [{ name: "TOML", extensions: ["toml"] }],
                });
                if (path) {
                  await config.exportConfig(path);
                  alert("配置导出成功");
                }
              } else {
                const path = await open({ filters: [{ name: "TOML", extensions: ["toml"] }] });
                if (path) {
                  await config.importConfig(path as string);
                  config.get().then(setCfg);
                  alert("配置导入成功，部分设置需重启生效");
                }
              }
            }}
            onCloud={async (dir) => {
              if (dir === "export") {
                await dataIO.exportConfigS3();
                alert("配置已导出到云端");
              } else {
                await dataIO.importConfigS3();
                config.get().then(setCfg);
                alert("云端配置导入成功，部分设置需重启生效");
              }
            }}
          />
        </Section>
      </Box>
    </ScrollArea>
  );
}

function DataIORow({
  label,
  onLocal,
  onCloud,
}: {
  label: string;
  onLocal: (dir: "export" | "import") => Promise<void>;
  onCloud: (dir: "export" | "import") => Promise<void>;
}) {
  const [pending, setPending] = useState<"export" | "import" | null>(null);

  const execute = async (target: "local" | "cloud") => {
    const dir = pending!;
    setPending(null);
    try {
      if (target === "local") await onLocal(dir);
      else await onCloud(dir);
    } catch (e) {
      alert(`${target === "cloud" ? "云端" : ""}${dir === "export" ? "导出" : "导入"}失败: ${e}`);
    }
  };

  return (
    <Group gap="1rem" py="0.65rem" wrap="nowrap">
      <Text size="sm" c="var(--color-label-secondary)" w={120} style={{ flexShrink: 0 }}>
        {label}
      </Text>
      <Box style={{ flex: 1 }}>
        {pending === null ? (
          <Group gap="0.5rem">
            <Button variant="default" size="xs" onClick={() => setPending("export")}>
              导出
            </Button>
            <Button variant="default" size="xs" onClick={() => setPending("import")}>
              导入
            </Button>
          </Group>
        ) : (
          <Group gap="0.5rem">
            <Text size="xs" c="var(--color-label-tertiary)">
              {pending === "export" ? "导出到" : "导入自"}:
            </Text>
            <Button variant="default" size="xs" onClick={() => execute("local")}>
              本地文件
            </Button>
            <Button variant="default" size="xs" onClick={() => execute("cloud")}>
              云端
            </Button>
            <ActionIcon variant="subtle" color="gray" onClick={() => setPending(null)}>
              ✕
            </ActionIcon>
          </Group>
        )}
      </Box>
    </Group>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <Box mb="lg">
      <Text
        size="xs"
        tt="uppercase"
        c="dimmed"
        fw={600}
        mb="xs"
        style={{ letterSpacing: "0.06em" }}
      >
        {title}
      </Text>
      <Stack gap="xs">{children}</Stack>
    </Box>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <Group gap="md" align="center" wrap="nowrap">
      <Text size="sm" c="dimmed" w={96} style={{ flexShrink: 0 }}>
        {label}
      </Text>
      <Box style={{ flex: 1 }}>{children}</Box>
    </Group>
  );
}
