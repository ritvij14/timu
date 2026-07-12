import { router } from 'expo-router';
import { StyleSheet, Text, View } from 'react-native';

import { Button } from '@/components/onboarding/button';
import { ErrorCard } from '@/components/onboarding/error-card';
import { ScreenShell } from '@/components/onboarding/screen-shell';
import { StepIndicator } from '@/components/onboarding/step-indicator';
import { useTheme } from '@/hooks/use-theme';
import { Fonts, Spacing } from '@/constants/theme';

type PairErrorVariant = 'expired' | 'unreachable';

const VARIANT: PairErrorVariant = 'unreachable';

export default function PairErrorScreen() {
  const theme = useTheme();

  const steps = [
    { label: 'Connecting to mac-mini-home', status: 'error' as const },
    { label: 'Generating device key', status: 'pending' as const },
    { label: 'Provisioning access', status: 'pending' as const },
    { label: 'Checking machine', status: 'pending' as const },
  ];

  return (
    <ScreenShell
      title="Couldn’t pair"
      onBack={() => router.back()}>
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

        {VARIANT === 'expired' ? (
          <ErrorCard
            title="This code expired"
            message="Pairing codes last about 2 minutes. Run the command again for a fresh one."
            command="npx yourapp pair"
            actionTitle="Copy"
            onAction={() => {}}
            severity="warning"
          />
        ) : (
          <ErrorCard
            title="Can’t reach mac-mini-home"
            message="Make sure your phone and computer are on the same Wi-Fi network, then try again."
            severity="error"
          />
        )}
      </View>

      <View style={styles.footer}>
        <Button title="Try again" onPress={() => {}} />
        {VARIANT === 'unreachable' && (
          <Button
            title="Scan a new code"
            variant="ghost"
            onPress={() => {}}
          />
        )}
        {VARIANT === 'expired' && (
          <Button
            title="Scan again"
            variant="ghost"
            onPress={() => {}}
          />
        )}
      </View>
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
  footer: {
    marginTop: 'auto',
    gap: Spacing.two,
    paddingTop: Spacing.four,
  },
});
