import { StyleSheet, Text, View } from 'react-native';

import { useTheme } from '@/hooks/use-theme';
import { Fonts, Spacing } from '@/constants/theme';

type Status = 'ready' | 'missing';

type StatusRowProps = {
  label: string;
  status: Status;
};

export function StatusRow({ label, status }: StatusRowProps) {
  const theme = useTheme();
  const isReady = status === 'ready';

  return (
    <View style={styles.row}>
      <Text style={[styles.label, { color: theme.text }]} numberOfLines={1}>
        {label}
      </Text>
      <View style={styles.right}>
        <View
          style={[
            styles.dot,
            { backgroundColor: isReady ? theme.success : theme.warning },
          ]}
        />
        <Text
          style={[
            styles.statusText,
            { color: isReady ? theme.success : theme.warning },
          ]}
          numberOfLines={1}>
          {isReady ? 'Ready' : 'Missing'}
        </Text>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: Spacing.two,
  },
  label: {
    fontSize: 16,
    fontWeight: '500',
    fontFamily: Fonts.sans,
  },
  right: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: Spacing.two,
  },
  dot: {
    width: 8,
    height: 8,
    borderRadius: 4,
  },
  statusText: {
    fontSize: 14,
    fontWeight: '600',
    fontFamily: Fonts.sans,
  },
});
