import { Link, router } from 'expo-router';
import { StyleSheet, Text, View } from 'react-native';
import { SymbolView } from 'expo-symbols';

import { Button } from '@/components/onboarding/button';
import { ScreenShell } from '@/components/onboarding/screen-shell';
import { useTheme } from '@/hooks/use-theme';
import { Fonts, Spacing } from '@/constants/theme';

export default function PairedSuccessScreen() {
  const theme = useTheme();

  return (
    <ScreenShell title="Paired" showBack={false}>
      <View style={styles.center}>
        <View
          style={[
            styles.iconCircle,
            { backgroundColor: theme.success + '20' },
          ]}>
          <SymbolView
            name="checkmark.circle.fill"
            type="hierarchical"
            tintColor={theme.success}
            size={40}
            weight="regular"
            fallback={
              <Text style={[styles.fallbackIcon, { color: theme.success }]} numberOfLines={1}>
                ✓
              </Text>
            }
          />
        </View>

        <Text style={[styles.heading, { color: theme.text }]}>Paired with mac-mini-home</Text>
        <Text style={[styles.subtitle, { color: theme.textSecondary }]} numberOfLines={3}>
          Your device key is stored securely on this phone. It won’t need the pairing code again.
        </Text>
      </View>

      <View style={styles.footer}>
        <Link href="/readiness" asChild>
          <Button title="Continue" />
        </Link>
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
    width: 88,
    height: 88,
    borderRadius: 44,
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: Spacing.two,
  },
  fallbackIcon: {
    fontSize: 40,
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
