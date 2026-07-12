import { Platform, ScrollView, StyleSheet, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { ScreenHeader } from './screen-header';

import { useTheme } from '@/hooks/use-theme';
import { BottomTabInset, MaxContentWidth, Spacing } from '@/constants/theme';

type ScreenShellProps = {
  title: string;
  onBack?: () => void;
  showBack?: boolean;
  children?: React.ReactNode;
  footer?: React.ReactNode;
  scrollable?: boolean;
};

export function ScreenShell({
  title,
  onBack,
  showBack,
  children,
  footer,
  scrollable = true,
}: ScreenShellProps) {
  const theme = useTheme();
  const insets = useSafeAreaInsets();

  const bottomPadding =
    Platform.OS === 'ios'
      ? Math.max(insets.bottom, BottomTabInset)
      : insets.bottom + BottomTabInset;

  const content = (
    <View style={[styles.container, { backgroundColor: theme.background }]}>
      <ScreenHeader title={title} onBack={onBack} showBack={showBack} />

      <View style={styles.body}>
        {scrollable ? (
          <ScrollView
            contentContainerStyle={[
              styles.scrollContent,
              { paddingBottom: bottomPadding },
            ]}>
            {children}
          </ScrollView>
        ) : (
          <View style={[styles.staticContent, { paddingBottom: bottomPadding }]}>{children}</View>
        )}
      </View>

      {footer && (
        <View
          style={[
            styles.footer,
            {
              backgroundColor: theme.background,
              paddingBottom: bottomPadding,
              borderTopColor: theme.border,
            },
          ]}>
          <View style={styles.footerInner}>{footer}</View>
        </View>
      )}
    </View>
  );

  return content;
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
  body: {
    flex: 1,
  },
  scrollContent: {
    flexGrow: 1,
    paddingHorizontal: Spacing.three,
    paddingTop: Spacing.two,
    maxWidth: MaxContentWidth,
    alignSelf: 'center',
    width: '100%',
  },
  staticContent: {
    flex: 1,
    paddingHorizontal: Spacing.three,
    paddingTop: Spacing.two,
    maxWidth: MaxContentWidth,
    alignSelf: 'center',
    width: '100%',
  },
  footer: {
    borderTopWidth: StyleSheet.hairlineWidth,
    paddingTop: Spacing.three,
    paddingHorizontal: Spacing.three,
  },
  footerInner: {
    maxWidth: MaxContentWidth,
    alignSelf: 'center',
    width: '100%',
  },
});
