import { useState } from 'react';
import { Link, router } from 'expo-router';
import { Pressable, StyleSheet, Text, View } from 'react-native';
import { SymbolView } from 'expo-symbols';

import { Button } from '@/components/onboarding/button';
import { CodeCopyRow } from '@/components/onboarding/code-copy-row';
import { ScreenShell } from '@/components/onboarding/screen-shell';
import { useTheme } from '@/hooks/use-theme';
import { Fonts, Spacing } from '@/constants/theme';

export default function PairScreen() {
  const theme = useTheme();
  const [scanning, setScanning] = useState(false);

  return (
    <ScreenShell
      title="Pair a machine"
      onBack={() => router.back()}
      footer={
        scanning ? (
          <View style={styles.scanningFooter}>
            <View style={[styles.pulse, { backgroundColor: theme.primary }]} />
            <Text style={[styles.scanningText, { color: theme.textSecondary }]}>
              Looking for a QR code…
            </Text>
          </View>
        ) : undefined
      }>
      <View style={styles.content}>
        <CodeCopyRow
          label="Run on your computer"
          command="npx yourapp pair"
          onCopy={() => {}}
        />

        <View style={[styles.scanner, { borderColor: theme.border }]}>
          <Pressable
            style={styles.scannerInner}
            onPress={() => setScanning((s) => !s)}>
            <View style={styles.cornerGroup}>
              <View
                style={[
                  styles.corner,
                  styles.cornerTL,
                  { borderColor: theme.primary },
                ]}
              />
              <View
                style={[
                  styles.corner,
                  styles.cornerTR,
                  { borderColor: theme.primary },
                ]}
              />
            </View>

            <SymbolView
              name="qrcode"
              type="hierarchical"
              tintColor={theme.textSecondary}
              size={48}
              weight="regular"
              fallback={
                <Text style={[styles.fallbackIcon, { color: theme.textSecondary }]}>▣</Text>
              }
            />

            <View style={styles.cornerGroup}>
              <View
                style={[
                  styles.corner,
                  styles.cornerBL,
                  { borderColor: theme.primary },
                ]}
              />
              <View
                style={[
                  styles.corner,
                  styles.cornerBR,
                  { borderColor: theme.primary },
                ]}
              />
            </View>
          </Pressable>
        </View>

        <Text style={[styles.helper, { color: theme.textSecondary }]} numberOfLines={2}>
          Command stays visible + copyable in case they haven’t run it yet — no separate instructions screen.
        </Text>
      </View>

      <Link href="/pair-camera-denied" style={{ opacity: 0 }} />
    </ScreenShell>
  );
}

const styles = StyleSheet.create({
  content: {
    gap: Spacing.four,
    paddingTop: Spacing.three,
  },
  scanner: {
    borderWidth: 1,
    borderStyle: 'dashed',
    borderRadius: 20,
    aspectRatio: 1,
    padding: Spacing.four,
  },
  scannerInner: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: Spacing.six,
  },
  cornerGroup: {
    width: '100%',
    flexDirection: 'row',
    justifyContent: 'space-between',
  },
  corner: {
    width: 28,
    height: 28,
    borderColor: 'transparent',
    borderWidth: 3,
  },
  cornerTL: {
    borderTopWidth: 3,
    borderLeftWidth: 3,
    borderTopLeftRadius: 8,
  },
  cornerTR: {
    borderTopWidth: 3,
    borderRightWidth: 3,
    borderTopRightRadius: 8,
  },
  cornerBL: {
    borderBottomWidth: 3,
    borderLeftWidth: 3,
    borderBottomLeftRadius: 8,
  },
  cornerBR: {
    borderBottomWidth: 3,
    borderRightWidth: 3,
    borderBottomRightRadius: 8,
  },
  fallbackIcon: {
    fontSize: 48,
  },
  helper: {
    fontSize: 13,
    lineHeight: 18,
    textAlign: 'center',
    fontFamily: Fonts.sans,
  },
  scanningFooter: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: Spacing.two,
  },
  pulse: {
    width: 8,
    height: 8,
    borderRadius: 4,
  },
  scanningText: {
    fontSize: 14,
    fontFamily: Fonts.sans,
  },
});
