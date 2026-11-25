import { createContext, useContext } from 'react';

export const DragDisabledContext = createContext(false);

export function useDragDisabled() {
  return useContext(DragDisabledContext);
}
