import { Link, router } from 'expo-router';
import { Pressable, StyleSheet, Text, View } from 'react-native';

import { Button } from '@/components/onboarding/button';
import { ScreenShell } from '@/components/onboarding/screen-shell';
import { StepIndicator } from '@/components/onboarding/step-indicator';
import { useTheme } from '@/hooks/use-theme';
import { Fonts, Spacing } from '@/constants/theme';

export default function PairingProgressScreen() {
  const theme = useTheme();

  const steps = [
    { label: 'Connecting to mac-mini-home', status: 'completed' as const },
    { label: 'Generating device key', status: 'completed' as const },
    { label: 'Provisioning access', status: 'current' as const },
    { label: 'Checking machine', status: 'pending' as const },
  ];

  return (
    <ScreenShell
      title="Pairing…"
      showBack={false}
      footer={
        <Pressable onPress={() => {}}>
          <Text style={[styles.cancel, { color: theme.textSecondary }]} numberOfLines={1}>
            Cancel
          </Text>
        </Pressable>
      }>
      <View style={styles.content}>
        <View style={[styles.card, { backgroundColor: theme.backgroundElement }]}>
          <View style={styles.cardHeader}>
            <Text style={[styles.machineName, { color: theme.text }]} numberOfLines={1}>
              mac-mini-home
            </Text>
            <View
              style={[styles.pill, { backgroundColor: theme.backgroundSelected }]}>
              <Text style={[styles.pillText, { color: theme.textSecondary }]} numberOfLines={1}>
                macOS
              </Text>
            </View>
          </View>

          <StepIndicator steps={steps} />
        </View>

        <Text style={[styles.hint, { color: theme.textSecondary }]} numberOfLines={2}>
          One-time setup — your phone won’t need this again.
        </Text>
      </View>

      <Link href="/pair-error" style={{ opacity: 0 }} />
    </ScreenShell>
  );
}

const styles = StyleSheet.create({
  content: {
    gap: Spacing.four,
    paddingTop: Spacing.two,
  },
  card: {
    borderRadius: 16,
    padding: Spacing.three,
    gap: Spacing.four,
  },
  cardHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: Spacing.two,
  },
  machineName: {
    flex: 1,
    fontSize: 17,
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
  hint: {
    fontSize: 13,
    lineHeight: 18,
    textAlign: 'center',
    fontFamily: Fonts.sans,
  },
  cancel: {
    textAlign: 'center',
    fontSize: 15,
    fontWeight: '500',
    fontFamily: Fonts.sans,
  },
});
