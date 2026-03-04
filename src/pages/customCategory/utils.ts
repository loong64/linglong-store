type Category = API.APP.AppCategories;

/**
 * 生成空分类列表
 * @param count 生成数量
 * @returns 空分类数据数组
 */
export const generateEmptyCategories = (count: number): Category[] => {
  return Array.from(
    { length: count },
    (_, index) =>
      ({
        id: `empty-cat-${index}`,
        categoryId: `empty-cat-id-${index}`,
        categoryName: '',
      } as Category),
  )
}
