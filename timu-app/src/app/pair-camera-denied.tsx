import { router } from 'expo-router';
import { StyleSheet, Text, View } from 'react-native';
import { SymbolView } from 'expo-symbols';

import { Button } from '@/components/onboarding/button';
import { ScreenShell } from '@/components/onboarding/screen-shell';
import { useTheme } from '@/hooks/use-theme';
import { Fonts, Spacing } from '@/constants/theme';

export default function PairCameraDeniedScreen() {
  const theme = useTheme();

  return (
    <ScreenShell
      title="Pair a machine"
      onBack={() => router.back()}>
      <View style={styles.center}>
        <View
          style={[
            styles.iconCircle,
            { backgroundColor: theme.backgroundElement },
          ]}>
          <SymbolView
            name="xmark.circle"
            type="hierarchical"
            tintColor={theme.danger}
            size={32}
            weight="regular"
            fallback={
              <Text style={[styles.fallbackIcon, { color: theme.danger }]} numberOfLines={1}>
                📷🚫
              </Text>
            }
          />
        </View>

        <Text style={[styles.heading, { color: theme.text }]}>Camera access needed</Text>
        <Text style={[styles.subtitle, { color: theme.textSecondary }]} numberOfLines={3}>
          Turn on camera access to scan the pairing code.
        </Text>
      </View>

      <View style={styles.footer}>
        <Button title="Open Settings" onPress={() => {}} />
      </View>
    </ScreenShell>
  );
}

const styles = StyleSheet.create({
  center: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    gap: Spacing.three,
    paddingHorizontal: Spacing.four,
    marginTop: -Spacing.six,
  },
  iconCircle: {
    width: 72,
    height: 72,
    borderRadius: 36,
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: Spacing.two,
  },
  fallbackIcon: {
    fontSize: 24,
  },
  heading: {
    fontSize: 22,
    fontWeight: '600',
    textAlign: 'center',
    fontFamily: Fonts.sans,
  },
  subtitle: {
    fontSize: 15,
    lineHeight: 22,
    textAlign: 'center',
    fontFamily: Fonts.sans,
  },
  footer: {
    marginTop: 'auto',
    paddingTop: Spacing.four,
  },
});
