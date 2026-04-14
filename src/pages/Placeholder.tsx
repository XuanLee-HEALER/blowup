import { Center, Stack, Text } from "@mantine/core";

interface PlaceholderProps {
  title: string;
  milestone: string;
}

export default function Placeholder({ title, milestone }: PlaceholderProps) {
  return (
    <Center h="100%">
      <Stack gap="xs" align="center">
        <Text fw={600} c="var(--color-label-tertiary)">
          {title}
        </Text>
        <Text size="sm" c="var(--color-label-quaternary)">
          {milestone} 中实现
        </Text>
      </Stack>
    </Center>
  );
}
