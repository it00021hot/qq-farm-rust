declare namespace Api {
  namespace SystemManage {
    type CommonSearchParams = Pick<Common.PaginatingCommonParams, 'current' | 'size'>;

    type EnableStatus = 1 | 2;

    /** admin user */
    type Admin = {
      id: number;
      uuid: string;
      nickName: string;
      realName: string;
      account: string;
      phone: string;
      email: string;
      roleIds: string;
      status: EnableStatus;
      createdAt: number;
      updatedAt: number;
    };

    type AdminSearchParams = CommonType.RecordNullable<
      Pick<Admin, 'status'> & CommonSearchParams & { keyword?: string }
    >;

    type AdminList = Common.PaginatingQueryRecord<Admin>;

    type AdminCreateParams = {
      account: string;
      password: string;
      nickName: string;
      realName?: string;
      phone?: string;
      email?: string;
      roleIds: string;
      status: EnableStatus;
    };

    type AdminUpdateParams = {
      id: number;
      nickName: string;
      realName?: string;
      phone?: string;
      email?: string;
      roleIds: string;
      status: EnableStatus;
      password?: string;
    };
  }
}
