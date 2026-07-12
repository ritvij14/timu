import { router } from 'expo-router';
import { StyleSheet, Text, View } from 'react-native';

import { Button } from '@/components/onboarding/button';
import { CodeCopyRow } from '@/components/onboarding/code-copy-row';
import { ErrorCard } from '@/components/onboarding/error-card';
import { ScreenShell } from '@/components/onboarding/screen-shell';
import { StatusRow } from '@/components/onboarding/status-row';
import { useTheme } from '@/hooks/use-theme';
import { Fonts, Spacing } from '@/constants/theme';

export default function ReadinessScreen() {
  const theme = useTheme();

  const tools = [
    { label: 'tmux', status: 'missing' as const },
    { label: 'git', status: 'ready' as const },
    { label: 'node', status: 'ready' as const },
    { label: 'pnpm', status: 'ready' as const },
    { label: 'codex', status: 'ready' as const },
    { label: 'claude', status: 'missing' as const },
  ];

  const missingRequired = true;

  return (
    <ScreenShell
      title="mac-mini-home"
      onBack={() => router.back()}>
      <View style={styles.content}>
        <View style={styles.headingBlock}>
          <View style={styles.titleRow}>
            <Text style={[styles.title, { color: theme.text }]} numberOfLines={1}>
              Ready to go — almost
            </Text>
            <View
              style={[styles.pill, { backgroundColor: theme.backgroundSelected }]}>
              <Text style={[styles.pillText, { color: theme.textSecondary }]} numberOfLines={1}>
                macOS
              </Text>
            </View>
          </View>
          <Text style={[styles.subtitle, { color: theme.textSecondary }]} numberOfLines={2}>
            One thing needs attention before you can start a session.
          </Text>
        </View>

        <View style={[styles.card, { backgroundColor: theme.backgroundElement }]}>
          {tools.map((tool) => (
            <StatusRow key={tool.label} label={tool.label} status={tool.status} />
          ))}
        </View>

        <ErrorCard
          title="TMUX MISSING"
          message="tmux is needed to keep coding sessions alive after you close the app."
          command="brew install tmux"
          actionTitle="Copy"
          severity="warning"
        />
      </View>

      <View style={styles.footer}>
        <Button title="Recheck" variant="secondary" onPress={() => {}} />
        <Button title="Continue" disabled={missingRequired} onPress={() => {}} />
      </View>
    </ScreenShell>
  );
}

const styles = StyleSheet.create({
  content: {
    gap: Spacing.four,
    paddingTop: Spacing.two,
  },
  headingBlock: {
    gap: Spacing.two,
  },
  titleRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: Spacing.two,
  },
  title: {
    fontSize: 22,
    fontWeight: '600',
    fontFamily: Fonts.sans,
  },
  pill: {
    borderRadius: 6,
    paddingHorizontal: Spacing.two,
    paddingVertical: Spacing.half,
  },
  pillText: {
    fontSize: 12,
    fontWeight: '600',
    fontFamily: Fonts.sans,
  },
  subtitle: {
    fontSize: 15,
    lineHeight: 22,
    fontFamily: Fonts.sans,
  },
  card: {
    borderRadius: 16,
    paddingHorizontal: Spacing.three,
    paddingVertical: Spacing.two,
  },
  footer: {
    marginTop: 'auto',
    gap: Spacing.two,
    paddingTop: Spacing.four,
  },
});
