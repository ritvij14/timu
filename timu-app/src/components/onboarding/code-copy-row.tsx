import { Pressable, StyleSheet, Text, View } from 'react-native';

import { useTheme } from '@/hooks/use-theme';
import { Fonts, Spacing } from '@/constants/theme';

type CodeCopyRowProps = {
  command: string;
  label?: string;
  onCopy?: () => void;
};

export function CodeCopyRow({ command, label, onCopy }: CodeCopyRowProps) {
  const theme = useTheme();

  return (
    <View style={styles.wrapper}>
      {label && (
        <Text style={[styles.label, { color: theme.textSecondary }]} numberOfLines={1}>
          {label}
        </Text>
      )}
      <View style={[styles.row, { backgroundColor: theme.backgroundElement }]}>
        <Text style={[styles.command, { color: theme.text }]} numberOfLines={1}>
          <Text style={{ color: theme.primary }}>$</Text> {command}
        </Text>
        <Pressable onPress={onCopy} style={styles.copyButton}>
          <Text style={[styles.copyText, { color: theme.primary }]} numberOfLines={1}>
            Copy
          </Text>
        </Pressable>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  wrapper: {
    gap: Spacing.two,
  },
  label: {
    fontSize: 12,
    fontWeight: '600',
    textTransform: 'uppercase',
    letterSpacing: 0.5,
    fontFamily: Fonts.sans,
  },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: Spacing.three,
    paddingVertical: Spacing.three,
    borderRadius: 12,
    gap: Spacing.three,
  },
  command: {
    flex: 1,
    fontSize: 15,
    fontWeight: '500',
    fontFamily: Fonts.mono,
  },
  copyButton: {
    paddingVertical: Spacing.half,
    paddingHorizontal: Spacing.two,
  },
  copyText: {
    fontSize: 14,
    fontWeight: '600',
    fontFamily: Fonts.sans,
  },
});
