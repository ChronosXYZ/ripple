declare module 'react-jdenticon' {
  import * as React from 'react';

  interface JdenticonProps {
    value?: string;
    size?: number | string;
    className?: string;
    [key: string]: any;
  }

  const Jdenticon: React.FC<JdenticonProps>;
  export default Jdenticon;
}